# frozen_string_literal: true

require 'json'
require 'socket'

module Transmute
  module Rspec
    class Client
      def initialize(socket_path: Server::DEFAULT_SOCKET)
        @socket_path = socket_path
      end

      def run(spec_path)
        request({ action: 'run', spec: spec_path })
      end

      def ping
        request({ action: 'ping' })
      end

      private

      def request(payload)
        open_socket do |sock|
          sock.puts(JSON.generate(payload))
          line = sock.gets
          if line.nil? || line.empty?
            raise "transmute-rspec daemon closed the connection without responding (it likely crashed; check the daemon log)"
          end

          JSON.parse(line)
        end
      end

      def open_socket(&block)
        if @socket_path.start_with?('tcp://')
          host, port = Server.parse_tcp(@socket_path)
          TCPSocket.open(host, port, &block)
        else
          UNIXSocket.open(@socket_path, &block)
        end
      end
    end
  end
end
