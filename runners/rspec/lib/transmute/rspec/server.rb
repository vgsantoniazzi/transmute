# frozen_string_literal: true

require 'json'
require 'socket'
require 'stringio'
require 'rspec/core'

module Transmute
  module Rspec
    class Server
      DEFAULT_SOCKET = '/tmp/transmute-rspec.sock'

      def initialize(socket_path: DEFAULT_SOCKET, logger: $stderr, fork: false, allow_public: false, allow_plain_ruby: false)
        @socket_path = socket_path
        @logger = logger
        @fork = fork
        @allow_public = allow_public
        @allow_plain_ruby = allow_plain_ruby
        @file_mtimes = {}
        @started_at = Time.now
        @requests_handled = 0
        @shutdown = false
        @current_child_pid = nil
      end

      def serve
        guard_against_plain_ruby_in_process_mode

        server = nil
        server = build_server(@socket_path)
        install_signal_handlers
        log_startup_banner

        until @shutdown
          ready = IO.select([server], nil, nil, 0.25)
          next unless ready

          begin
            client = server.accept_nonblock
          rescue IO::WaitReadable, Errno::EINTR
            next
          end
          handle(client)
          client.close
        end

        @logger.puts 'transmute-rspec: shutting down'
      ensure
        begin
          server.close if server && !server.closed?
        rescue StandardError
          nil
        end
        File.delete(@socket_path) if @socket_path && !@socket_path.start_with?('tcp://') && File.exist?(@socket_path)
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

      def self.parse_tcp(uri)
        body = uri.sub(%r{^tcp://}, '')
        host, port = body.split(':')
        [host, Integer(port)]
      end

      def handle(client)
        while (line = client.gets)
          response = begin
            dispatch(parse(line))
          rescue StandardError, ScriptError => e
            error_response(e)
          end
          begin
            client.puts(JSON.generate(response))
          rescue Errno::EPIPE, Errno::ECONNRESET, IOError => e
            @logger.puts "transmute-rspec: client disconnected mid-response (#{e.class}); skipping"
            return
          end
          @requests_handled += 1
        end
      rescue Errno::EPIPE, Errno::ECONNRESET, IOError => e
        @logger.puts "transmute-rspec: client connection error (#{e.class}); continuing"
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
        return missing_spec_response(spec_path) unless spec_path && !spec_path.empty?

        resolved = resolve_spec_path(spec_path)
        return resolved unless resolved.is_a?(String)

        return missing_spec_response(resolved) unless File.exist?(resolved)

        ensure_spec_root_on_load_path(resolved)

        if @fork
          run_spec_in_fork(resolved)
        else
          run_spec_in_process(resolved)
        end
      end

      def resolve_spec_path(spec_path)
        absolute = File.expand_path(spec_path)
        project_root = File.expand_path(Dir.pwd)
        prefix = "#{project_root}/"
        unless absolute == project_root || absolute.start_with?(prefix)
          return {
            exit_code: 2,
            stdout: "transmute-rspec: refusing spec path outside project root: #{spec_path} (resolved #{absolute}, project #{project_root})\n"
          }
        end

        absolute
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
        begin
          pid = Process.fork do
            reader.close
            begin
              response = run_spec_in_fork_child(spec_path)
              writer.write(JSON.generate(response))
            rescue StandardError, ScriptError => e
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
          @current_child_pid = nil
          if raw.nil? || raw.empty?
            return {
              exit_code: 2,
              stdout: "transmute-rspec: forked worker died (#{status.exitstatus ? "exit #{status.exitstatus}" : "signal #{status.termsig}"})\n"
            }
          end

          JSON.parse(raw)
        ensure
          reader.close unless reader.closed?
          writer.close unless writer.closed?
        end
      end

      def run_spec_in_fork_child(spec_path)
        reset_database_connections
        captured = StringIO.new
        ::RSpec.configuration.output_stream = captured
        ::RSpec.configuration.error_stream = captured
        reset_rspec_world
        Kernel.load(spec_path)
        exit_code = ::RSpec::Core::Runner.run([], captured, captured)
        { exit_code: exit_code, stdout: captured.string }
      end

      def reset_database_connections
        return unless defined?(::ActiveRecord::Base)

        begin
          if ::ActiveRecord::Base.connection_handler.respond_to?(:clear_all_connections!)
            ::ActiveRecord::Base.connection_handler.clear_all_connections!
          end
        rescue StandardError => e
          @logger.puts "transmute-rspec: AR clear_all_connections! failed: #{e.class}: #{e.message}"
        end
        begin
          ::ActiveRecord::Base.establish_connection
        rescue StandardError => e
          @logger.puts "transmute-rspec: AR establish_connection failed: #{e.class}: #{e.message}"
        end
      end

      def missing_spec_response(spec_path)
        { exit_code: 2, stdout: "transmute-rspec: spec file not found: #{spec_path}\n" }
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

      private

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
        cwd = "#{Dir.pwd}/"
        $LOADED_FEATURES.each do |path|
          next unless path.start_with?(cwd)
          next unless File.file?(path)

          mtime = File.mtime(path)
          last = @file_mtimes[path]
          if last.nil? || mtime > last
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
