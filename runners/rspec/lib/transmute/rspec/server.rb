# frozen_string_literal: true

require 'json'
require 'socket'
require 'stringio'
require 'rspec/core'

module Transmute
  module Rspec
    class Server
      DEFAULT_SOCKET = '/tmp/transmute-rspec.sock'
      DEFAULT_READ_TIMEOUT = 10
      ROTATION_HINT_THRESHOLD = 500

      # A request line is a tiny JSON object ({"action":"run","spec":"..."}); a
      # path can be long but never megabytes. Capping the byte-at-a-time reader
      # stops a client that streams a newline-less flood from growing the buffer
      # without bound (loopback throughput is high) and ballooning daemon RSS.
      MAX_REQUEST_LINE_BYTES = 64 * 1024

      # Raised when an accepted client fails to deliver a usable request line:
      # either it stays silent past @read_timeout, or it floods past
      # MAX_REQUEST_LINE_BYTES with no newline. A control-flow sentinel so the
      # happy path in #handle stays a plain `while (line = read_request_line)`;
      # #handle logs its message and drops just that connection, still serving.
      class ClientReadTimeout < StandardError; end

      def initialize(socket_path: DEFAULT_SOCKET, logger: $stderr, fork: false, allow_public: false,
                     allow_plain_ruby: false, read_timeout: DEFAULT_READ_TIMEOUT)
        @socket_path = socket_path
        @logger = logger
        @fork = fork
        @allow_public = allow_public
        @allow_plain_ruby = allow_plain_ruby
        @read_timeout = read_timeout
        @project_root = File.expand_path(Dir.pwd)
        @file_mtimes = {}
        @started_at = Time.now
        @requests_handled = 0
        @shutdown = false
        @current_child_pid = nil
      end

      def serve
        guard_against_plain_ruby_in_process_mode

        server = build_server(@socket_path)
        install_signal_handlers
        log_startup_banner

        accept_loop(server)

        @logger.puts 'transmute-rspec: shutting down'
      ensure
        begin
          server.close if server && !server.closed?
        rescue StandardError
          # A signal handler or a failed accept may have already torn the socket
          # down; swallow so close failure can't mask the real exception or skip
          # the socket-file cleanup below.
          nil
        end
        File.delete(@socket_path) if @socket_path && !@socket_path.start_with?('tcp://') && File.exist?(@socket_path)
      end

      def within_project_root?(absolute_path)
        absolute_path == project_root || absolute_path.start_with?("#{project_root}/")
      end

      def self.parse_tcp(uri)
        body = uri.sub(%r{^tcp://}, '')
        if (match = body.match(/\A\[(?<host>.+)\]:(?<port>\d+)\z/))
          return [match[:host], Integer(match[:port])]
        end

        host, _colon, port = body.rpartition(':')
        [host, Integer(port)]
      end

      private

      def accept_loop(server)
        until @shutdown
          next unless IO.select([server], nil, nil, 0.25)

          begin
            client = server.accept_nonblock
          rescue IO::WaitReadable, Errno::EINTR
            next
          end
          handle(client)
          client.close
        end
      end

      def build_server(path)
        if path.start_with?('tcp://')
          host, port = self.class.parse_tcp(path)
          guard_against_public_bind(host)
          TCPServer.new(host, port)
        else
          File.delete(path) if File.exist?(path)
          previous_umask = File.umask(0o077)
          begin
            UNIXServer.new(path)
          ensure
            File.umask(previous_umask)
          end
        end
      end

      def handle(client)
        while (line = read_request_line(client))
          response = begin
            dispatch(parse(line))
          rescue StandardError, ScriptError => e
            # ScriptError (SyntaxError/LoadError from Kernel.load on a mutated
            # spec) is not a StandardError, so a bare rescue would let it crash
            # the daemon instead of returning a clean error to the client.
            error_response(e)
          end
          begin
            client.puts(JSON.generate(response))
          rescue Errno::EPIPE, Errno::ECONNRESET, IOError => e
            @logger.puts "transmute-rspec: client disconnected mid-response (#{e.class}); skipping"
            return
          end
          note_request_handled
        end
      rescue ClientReadTimeout => e
        @logger.puts "transmute-rspec: #{e.message} from #{describe_peer(client)}; " \
                     'closing connection and continuing'
      rescue Errno::EPIPE, Errno::ECONNRESET, IOError => e
        @logger.puts "transmute-rspec: client connection error (#{e.class}); continuing"
      end

      # Reads one newline-terminated request, but never blocks the serial accept
      # loop longer than @read_timeout and never buffers more than
      # MAX_REQUEST_LINE_BYTES. A client that connects and stays silent (or
      # dribbles a partial line) would otherwise wedge every queued request
      # behind it forever; one that floods a newline-less stream would balloon
      # daemon RSS. Reads a byte at a time so it consumes exactly one line and
      # never over-reads into a following request. Returns the line, nil on clean
      # EOF, or raises ClientReadTimeout (silent client OR oversized line).
      def read_request_line(client)
        deadline = monotonic_now + @read_timeout
        buffer = +''
        loop do
          remaining = deadline - monotonic_now
          raise ClientReadTimeout, "read timeout after #{@read_timeout}s" if remaining <= 0
          raise ClientReadTimeout, "read timeout after #{@read_timeout}s" unless client.wait_readable(remaining)

          byte = client.read_nonblock(1, exception: false)
          return nil if byte.nil?
          next if byte == :wait_readable

          buffer << byte
          raise ClientReadTimeout, "request line exceeded #{MAX_REQUEST_LINE_BYTES} bytes" if buffer.bytesize > MAX_REQUEST_LINE_BYTES
          return buffer if byte == "\n"
        end
      end

      def monotonic_now
        Process.clock_gettime(Process::CLOCK_MONOTONIC)
      end

      def describe_peer(client)
        # Unix-socket peers carry no useful address; only TCP gives a host:port
        # worth naming in the log. peeraddr's arity differs between the two, so
        # branch on the socket type rather than rescuing an ArgumentError.
        return 'unix socket client' unless client.is_a?(TCPSocket)

        addr = client.peeraddr(false)
        "client #{addr[2]}:#{addr[1]}"
      rescue StandardError
        'client'
      end

      def note_request_handled
        @requests_handled += 1
        return unless @requests_handled == ROTATION_HINT_THRESHOLD

        # One-time nudge: long-lived Ruby processes accumulate leaked state, and
        # README tells operators to rotate "after a few hundred specs" but nothing
        # signals when. Fires once at the threshold so logs aren't flooded.
        @logger.puts "transmute-rspec: handled #{@requests_handled} requests; " \
                     'consider restarting the daemon to shed accumulated state'
      end

      def dispatch(request)
        case request['action']
        when 'run'
          run_spec(request['spec'])
        when 'ping'
          { exit_code: 0, stdout: 'pong' }
        when 'stats'
          stats_response
        when 'quit'
          @shutdown = true
          { exit_code: 0, stdout: 'bye' }
        else
          { exit_code: 2, stdout: "unknown action: #{request['action']}" }
        end
      end

      def run_spec(spec_path)
        return non_string_spec_response(spec_path) unless spec_path.is_a?(String)
        return missing_spec_response(spec_path) if spec_path.empty?

        absolute_path = File.expand_path(spec_path)
        return spec_outside_project_response(spec_path, absolute_path) unless within_project_root?(absolute_path)
        return missing_spec_response(absolute_path) unless File.exist?(absolute_path)

        # Re-check after realpath: File.expand_path does not resolve symlinks, so a
        # link under the root pointing outside would otherwise pass the guard and
        # Kernel.load external code. realpath needs the file to exist, hence the
        # order. Falls back to absolute_path if realpath races (file vanished).
        real_path = resolve_real_path(absolute_path)
        return spec_outside_project_response(spec_path, real_path) unless within_project_root?(real_path)

        ensure_spec_root_on_load_path(real_path)

        if @fork
          run_spec_in_fork(real_path)
        else
          run_spec_in_process(real_path)
        end
      end

      def resolve_real_path(absolute_path)
        File.realpath(absolute_path)
      rescue Errno::ENOENT
        absolute_path
      end

      def non_string_spec_response(spec_path)
        { exit_code: 2, stdout: "transmute-rspec: spec must be a string, got #{spec_path.class}\n" }
      end

      def missing_spec_response(spec_path)
        { exit_code: 2, stdout: "transmute-rspec: spec file not found: #{spec_path}\n" }
      end

      def spec_outside_project_response(requested_path, absolute_path)
        {
          exit_code: 2,
          stdout: "transmute-rspec: refusing spec path outside project root: #{requested_path} " \
                  "(resolved #{absolute_path}, project #{project_root})\n"
        }
      end

      def run_spec_in_process(spec_path)
        reload_application_code
        reset_rspec_world

        captured = StringIO.new
        ::RSpec.configuration.output_stream = captured
        ::RSpec.configuration.error_stream = captured
        Kernel.load(spec_path)
        exit_code = ::RSpec::Core::Runner.run([], captured, captured)

        { exit_code: exit_code, stdout: captured.string }
      end

      def reset_rspec_world
        ::RSpec.clear_examples
        ::RSpec.world.wants_to_quit = false if ::RSpec.world.respond_to?(:wants_to_quit=)
      end

      def run_spec_in_fork(spec_path)
        reader, writer = IO.pipe
        pid = nil
        begin
          pid = Process.fork do
            reader.close
            begin
              response = run_spec_in_fork_child(spec_path)
              writer.write(JSON.generate(response))
            rescue StandardError, ScriptError => e
              # ScriptError caught for the same reason as in #handle: a mutated
              # spec's SyntaxError/LoadError must come back as an error response,
              # not kill the child silently.
              writer.write(JSON.generate(error_response(e)))
            ensure
              writer.close
              Process.exit!(0)
            end
          end
          @current_child_pid = pid
          writer.close
          raw = reader.read
          _, status = Process.wait2(pid)
          pid = nil
          return dead_fork_response(status) if raw.nil? || raw.empty?

          JSON.parse(raw)
        ensure
          # Reap here too: if reader.read or wait2 above was interrupted, the happy
          # path is skipped and an unreaped child would zombie on a long-lived
          # daemon. pid is cleared once already reaped, so this never double-waits.
          reap_child(pid) if pid
          @current_child_pid = nil
          reader.close unless reader.closed?
          writer.close unless writer.closed?
        end
      end

      def reap_child(pid)
        Process.kill('TERM', pid)
        Process.wait(pid)
      rescue Errno::ESRCH, Errno::ECHILD
        nil
      end

      def dead_fork_response(status)
        cause = status.exitstatus ? "exit #{status.exitstatus}" : "signal #{status.termsig}"
        {
          exit_code: 2,
          stdout: "transmute-rspec: forked worker died (#{cause})\n"
        }
      end

      def run_spec_in_fork_child(spec_path)
        return db_reconnect_failed_response unless reset_database_connections

        captured = StringIO.new
        ::RSpec.configuration.output_stream = captured
        ::RSpec.configuration.error_stream = captured
        reset_rspec_world
        Kernel.load(spec_path)
        exit_code = ::RSpec::Core::Runner.run([], captured, captured)
        { exit_code: exit_code, stdout: captured.string }
      end

      def db_reconnect_failed_response
        {
          exit_code: 2,
          stdout: "transmute-rspec: database reconnect failed in forked worker; " \
                  "skipping spec to avoid a misleading result\n"
        }
      end

      # Returns false when the worker could not re-establish its DB connection, so
      # the caller can surface a visible infra error (exit 2) instead of running
      # the spec on a dead connection — a misleading kill/survivor is worse than a
      # visible failure the operator can see and act on.
      def reset_database_connections
        return true unless defined?(::ActiveRecord::Base)

        begin
          if ::ActiveRecord::Base.connection_handler.respond_to?(:clear_all_connections!)
            ::ActiveRecord::Base.connection_handler.clear_all_connections!
          end
        rescue StandardError => e
          @logger.puts "transmute-rspec: AR clear_all_connections! failed: #{e.class}: #{e.message}"
        end
        begin
          ::ActiveRecord::Base.establish_connection
          true
        rescue StandardError => e
          @logger.puts "transmute-rspec: AR establish_connection failed: #{e.class}: #{e.message}"
          false
        end
      end

      def ensure_spec_root_on_load_path(spec_path)
        dir = File.dirname(File.expand_path(spec_path))
        root = spec_root_for(dir) || dir
        $LOAD_PATH.unshift(root) unless $LOAD_PATH.include?(root)
      end

      def spec_root_for(dir)
        current = dir
        loop do
          return current if %w[spec tests test __tests__].include?(File.basename(current))

          parent = File.dirname(current)
          return nil if parent == current

          current = parent
        end
      end

      attr_reader :project_root

      def parse(line)
        JSON.parse(line.strip)
      rescue JSON::ParserError => e
        raise "invalid JSON request: #{e.message}"
      end

      def reload_application_code
        if defined?(::Rails) && ::Rails.respond_to?(:application) && ::Rails.application
          reloader = ::Rails.application.reloader
          reloader.reload! if reloader.respond_to?(:reload!)
          return
        end
        reload_changed_loaded_features
      end

      def reload_changed_loaded_features
        project_root_prefix = "#{project_root}/"
        $LOADED_FEATURES.each do |path|
          next unless path.start_with?(project_root_prefix)
          next unless File.file?(path)

          mtime = File.mtime(path)
          previous_mtime = @file_mtimes[path]
          if previous_mtime.nil? || mtime > previous_mtime
            Kernel.load(path)
            @file_mtimes[path] = mtime
          end
        end
      end

      def guard_against_plain_ruby_in_process_mode
        return if @fork
        return if @allow_plain_ruby
        return if defined?(::Rails) && ::Rails.respond_to?(:application) && ::Rails.application

        raise <<~MSG
          transmute-rspec: refusing to serve in in-process mode without Rails.
          Plain-Ruby projects hit Kernel.load + class-reopen issues that produce wrong results silently.
          Either:
            - Use --fork mode (recommended for plain Ruby)
            - Pass --allow-plain-ruby if you know what you're doing
            - --require ./config/environment if this is a Rails app that hasn't loaded yet
        MSG
      end

      def guard_against_public_bind(host)
        return if @allow_public
        return if %w[127.0.0.1 ::1 localhost].include?(host)

        raise <<~MSG
          transmute-rspec: refusing to bind on #{host}. The daemon executes Kernel.load(spec_path) on any path
          a client sends; binding outside loopback exposes arbitrary code execution to the network.
          Either:
            - Bind on 127.0.0.1 (recommended)
            - Use a Unix socket (--listen /tmp/transmute-rspec.sock)
            - Pass --allow-public if you understand the risk (e.g. inside a single-tenant container)
        MSG
      end

      def install_signal_handlers
        %w[INT TERM].each do |sig|
          Signal.trap(sig) do
            @shutdown = true
            if @current_child_pid
              begin
                Process.kill('TERM', @current_child_pid)
              rescue Errno::ESRCH
                nil
              end
            end
          end
        end
      end

      def log_startup_banner
        mode = @fork ? 'fork-per-request' : 'in-process'
        @logger.puts "transmute-rspec: pid=#{Process.pid} listening on #{@socket_path} (#{mode})"
        @logger.puts "transmute-rspec: rspec #{::RSpec::Core::Version::STRING} ready"
      end

      def stats_response
        {
          exit_code: 0,
          stdout: JSON.generate({
            pid: Process.pid,
            uptime_s: (Time.now - @started_at).round(1),
            requests_handled: @requests_handled,
            mode: @fork ? 'fork' : 'in-process',
            rails_loaded: defined?(::Rails) && ::Rails.respond_to?(:application) && !::Rails.application.nil?
          })
        }
      end

      def error_response(error)
        {
          exit_code: 2,
          stdout: "transmute-rspec server error: #{error.class}: #{error.message}\n#{error.backtrace&.first(5)&.join("\n")}"
        }
      end
    end
  end
end
