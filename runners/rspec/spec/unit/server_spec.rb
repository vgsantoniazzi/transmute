# frozen_string_literal: true

require 'spec_helper'
require 'transmute/rspec/server'

RSpec.describe Transmute::Rspec::Server do
  describe '.parse_tcp' do
    it 'parses an IPv4 host and port' do
      expect(described_class.parse_tcp('tcp://127.0.0.1:9876')).to eq(['127.0.0.1', 9876])
    end

    it 'parses a hostname and port' do
      expect(described_class.parse_tcp('tcp://localhost:9876')).to eq(['localhost', 9876])
    end

    it 'parses a bracketed IPv6 literal into a bare host TCPServer accepts' do
      expect(described_class.parse_tcp('tcp://[::1]:9876')).to eq(['::1', 9876])
    end
  end

  describe '#within_project_root?' do
    around do |example|
      Dir.mktmpdir('transmute-root') do |dir|
        FileUtils.mkdir_p(File.join(dir, 'project'))
        FileUtils.mkdir_p(File.join(dir, 'elsewhere'))
        original = Dir.pwd
        Dir.chdir(File.join(dir, 'project'))
        begin
          example.run
        ensure
          Dir.chdir(original)
        end
      end
    end

    it 'pins the project root to the working directory at construction time' do
      project_root = File.expand_path(Dir.pwd)
      spec_under_root = File.join(project_root, 'spec', 'user_spec.rb')
      server = described_class.new(allow_plain_ruby: true)

      Dir.chdir(File.expand_path('../elsewhere', project_root))

      expect(server.within_project_root?(spec_under_root)).to be(true)
    end
  end

  describe '#run_spec_in_fork_child when the DB reconnect fails' do
    before do
      base = Class.new do
        def self.connection_handler
          Object.new
        end

        def self.establish_connection(*)
          raise 'could not connect to database'
        end
      end
      stub_const('ActiveRecord::Base', base)
    end

    it 'returns an infra error (exit 2) instead of running the spec on a dead connection' do
      server = described_class.new(fork: true, logger: StringIO.new)
      loaded = false
      allow(Kernel).to receive(:load) { loaded = true }

      response = server.send(:run_spec_in_fork_child, '/tmp/whatever_spec.rb')

      expect(response[:exit_code]).to eq(2)
      expect(response[:stdout]).to include('database reconnect failed')
      expect(loaded).to be(false)
    end
  end

  describe '#note_request_handled rotation hint' do
    before { stub_const('Transmute::Rspec::Server::ROTATION_HINT_THRESHOLD', 3) }

    it 'logs the restart nudge exactly once, when the threshold is crossed' do
      log = StringIO.new
      server = described_class.new(allow_plain_ruby: true, logger: log)

      5.times { server.send(:note_request_handled) }

      expect(log.string.scan('consider restarting the daemon').size).to eq(1)
    end
  end

  describe '#read_request_line' do
    it 'reassembles a newline-terminated line read one byte at a time' do
      server = described_class.new(allow_plain_ruby: true, logger: StringIO.new)
      reader, writer = UNIXSocket.pair
      writer.write("{\"action\":\"ping\"}\n")

      expect(server.send(:read_request_line, reader)).to eq("{\"action\":\"ping\"}\n")
    ensure
      reader.close
      writer.close
    end

    it 'raises when the line exceeds the max length, before the buffer can grow unbounded' do
      stub_const('Transmute::Rspec::Server::MAX_REQUEST_LINE_BYTES', 16)
      server = described_class.new(allow_plain_ruby: true, logger: StringIO.new)
      reader, writer = UNIXSocket.pair
      writer.write('x' * 64)

      expect { server.send(:read_request_line, reader) }
        .to raise_error(Transmute::Rspec::Server::ClientReadTimeout, /request line exceeded/)
    ensure
      reader.close
      writer.close
    end
  end

  describe '#run_spec with a non-string spec value' do
    around do |example|
      Dir.mktmpdir('transmute-spec-type') do |dir|
        original = Dir.pwd
        Dir.chdir(dir)
        begin
          example.run
        ensure
          Dir.chdir(original)
        end
      end
    end

    it 'returns a clean "spec must be a string" error instead of a NoMethodError' do
      server = described_class.new(allow_plain_ruby: true, logger: StringIO.new)

      response = server.send(:run_spec, 123)

      expect(response[:exit_code]).to eq(2)
      expect(response[:stdout]).to include('spec must be a string')
    end
  end

  describe '#within_project_root? with a symlink that escapes the root' do
    around do |example|
      Dir.mktmpdir('transmute-symlink') do |dir|
        FileUtils.mkdir_p(File.join(dir, 'project', 'spec'))
        FileUtils.mkdir_p(File.join(dir, 'outside'))
        original = Dir.pwd
        Dir.chdir(File.join(dir, 'project'))
        begin
          example.run(@outside_target = File.join(dir, 'outside', 'evil_spec.rb'))
        ensure
          Dir.chdir(original)
        end
      end
    end

    it 'refuses a spec whose path is a symlink pointing outside the project root' do
      File.write(@outside_target, 'puts :evil')
      link = File.join(Dir.pwd, 'spec', 'innocent_spec.rb')
      File.symlink(@outside_target, link)
      server = described_class.new(allow_plain_ruby: true, logger: StringIO.new)
      loaded = false
      allow(Kernel).to receive(:load) { loaded = true }

      response = server.send(:run_spec, link)

      expect(response[:exit_code]).to eq(2)
      expect(response[:stdout]).to include('outside project root')
      expect(loaded).to be(false)
    end
  end
end
