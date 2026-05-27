# frozen_string_literal: true

require 'json'
require 'socket'
require 'stringio'
require 'rspec/core'

module Transmute
  module Rspec
    class Server
      DEFAULT_SOCKET = '/tmp/transmute-rspec.sock'

      def initialize(socket_path: DEFAULT_SOCKET, logger: $stderr)
        @socket_path = socket_path
        @logger = logger
        @file_mtimes = {}
      end

      def serve
        server = build_server(@socket_path)
        @logger.puts "transmute-rspec: listening on #{@socket_path}"
        @logger.puts "transmute-rspec: rspec #{::RSpec::Core::Version::STRING} ready"

        loop do
          client = server.accept
          handle(client)
          client.close
        end
      ensure
        File.delete(@socket_path) if @socket_path && !@socket_path.start_with?('tcp://') && File.exist?(@socket_path)
      end

      def build_server(path)
        if path.start_with?('tcp://')
          host, port = parse_tcp(path)
          TCPServer.new(host, port)
        else
          File.delete(path) if File.exist?(path)
          srv = UNIXServer.new(path)
          File.chmod(0o600, path)
          srv
        end
      end

      def self.parse_tcp(uri)
        body = uri.sub(%r{^tcp://}, '')
        host, port = body.split(':')
        [host, Integer(port)]
      end

      def parse_tcp(uri)
        self.class.parse_tcp(uri)
      end

      def handle(client)
        while (line = client.gets)
          response = begin
            dispatch(parse(line))
          rescue StandardError, ScriptError => e
            error_response(e)
          end
          client.puts(JSON.generate(response))
        end
      end

      def dispatch(request)
        case request['action']
        when 'run'
          run_spec(request['spec'])
        when 'ping'
          { exit_code: 0, stdout: 'pong' }
        when 'quit'
          { exit_code: 0, stdout: 'bye' }
        else
          { exit_code: 2, stdout: "unknown action: #{request['action']}" }
        end
      end

      def run_spec(spec_path)
        return missing_spec_response(spec_path) unless File.exist?(spec_path)

        ensure_spec_root_on_load_path(spec_path)
        reload_application_code

        captured = StringIO.new
        ::RSpec.clear_examples
        ::RSpec.configuration.output_stream = captured
        ::RSpec.configuration.error_stream = captured
        Kernel.load(spec_path)
        exit_code = ::RSpec::Core::Runner.run([], captured, captured)

        { exit_code: exit_code, stdout: captured.string }
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

      def error_response(error)
        {
          exit_code: 2,
          stdout: "transmute-rspec server error: #{error.class}: #{error.message}\n#{error.backtrace&.first(5)&.join("\n")}"
        }
      end
    end
  end
end
