# frozen_string_literal: true

require 'tmpdir'
require 'fileutils'
require 'socket'
require 'json'
require 'rbconfig'

module DaemonHelpers
  GEM_ROOT = File.expand_path('..', __dir__)
  DAEMON_BIN = File.expand_path('exe/transmute-rspec', GEM_ROOT)
  RUNNER_BIN = File.expand_path('exe/transmute-rspec-run', GEM_ROOT)
  LIB_PATH = File.expand_path('lib', GEM_ROOT)

  def with_temp_project
    Dir.mktmpdir('transmute-rspec-it') do |dir|
      original = Dir.pwd
      Dir.chdir(dir)
      FileUtils.mkdir_p(File.join(dir, 'spec'))
      begin
        yield dir
      ensure
        Dir.chdir(original)
      end
    end
  end

  def write_passing_spec(project_dir, name = 'pass_spec.rb')
    path = File.join(project_dir, 'spec', name)
    File.write(path, <<~RUBY)
      require 'rspec'
      RSpec.describe 'pass' do
        it { expect(1).to eq(1) }
      end
    RUBY
    path
  end

  def write_failing_spec(project_dir, name = 'fail_spec.rb')
    path = File.join(project_dir, 'spec', name)
    File.write(path, <<~RUBY)
      require 'rspec'
      RSpec.describe 'fail' do
        it { expect(1).to eq(2) }
      end
    RUBY
    path
  end

  def temp_socket(label)
    File.join(Dir.tmpdir, "transmute-rspec-it-#{Process.pid}-#{label}-#{Time.now.to_f}.sock")
  end

  def spawn_daemon(socket_path:, extra_args: [], cwd: Dir.pwd)
    log_path = "#{socket_path}.log"
    File.delete(log_path) if File.exist?(log_path)
    args = [
      RbConfig.ruby,
      '-I', LIB_PATH,
      DAEMON_BIN,
      'serve',
      '--listen', socket_path,
      *extra_args
    ]
    pid = Process.spawn(*args, chdir: cwd, out: log_path, err: log_path)
    wait_for_ready(socket_path, log_path, pid)
    DaemonHandle.new(pid: pid, socket: socket_path, log: log_path)
  end

  def wait_for_ready(socket_path, log_path, pid, timeout: 10)
    deadline = Time.now + timeout
    until Time.now > deadline
      if File.exist?(log_path) && File.read(log_path).include?('ready')
        return
      end
      begin
        Process.waitpid(pid, Process::WNOHANG)
      rescue Errno::ECHILD
        nil
      end
      sleep 0.05
    end
    raise "daemon never became ready (log: #{File.exist?(log_path) ? File.read(log_path) : 'no log'})"
  end

  def request(socket_path, payload)
    UNIXSocket.open(socket_path) do |sock|
      sock.puts(JSON.generate(payload))
      JSON.parse(sock.gets.to_s)
    end
  end

  DaemonHandle = Struct.new(:pid, :socket, :log, keyword_init: true) do
    def stop
      Process.kill('TERM', pid)
      begin
        Process.wait(pid)
      rescue Errno::ECHILD
        nil
      end
    rescue Errno::ESRCH
      nil
    ensure
      File.delete(socket) if File.exist?(socket) && !socket.start_with?('tcp://')
      File.delete(log) if File.exist?(log)
    end

    def log_contents
      File.exist?(log) ? File.read(log) : ''
    end
  end
end

RSpec.configure do |config|
  config.include DaemonHelpers
  config.expect_with(:rspec) { |c| c.syntax = :expect }
  config.formatter = :documentation
end
