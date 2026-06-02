# frozen_string_literal: true

require 'spec_helper'

RSpec.describe 'transmute-rspec daemon' do
  describe 'in-process mode' do
    it 'runs a passing spec and returns exit_code 0' do
      with_temp_project do |project|
        spec = write_passing_spec(project)
        daemon = spawn_daemon(socket_path: temp_socket('inproc-pass'), extra_args: ['--allow-plain-ruby'], cwd: project)
        begin
          response = request(daemon.socket, action: 'run', spec: spec)
          expect(response['exit_code']).to eq(0)
          expect(response['stdout']).to include('1 example, 0 failures')
        ensure
          daemon.stop
        end
      end
    end

    it 'returns nonzero exit_code for a failing spec' do
      with_temp_project do |project|
        spec = write_failing_spec(project)
        daemon = spawn_daemon(socket_path: temp_socket('inproc-fail'), extra_args: ['--allow-plain-ruby'], cwd: project)
        begin
          response = request(daemon.socket, action: 'run', spec: spec)
          expect(response['exit_code']).not_to eq(0)
          expect(response['stdout']).to include('1 example, 1 failure')
        ensure
          daemon.stop
        end
      end
    end

    it 'runs many specs back-to-back without false failures from wants_to_quit leakage' do
      with_temp_project do |project|
        spec = write_passing_spec(project)
        daemon = spawn_daemon(socket_path: temp_socket('inproc-many'), extra_args: ['--allow-plain-ruby'], cwd: project)
        begin
          5.times do |i|
            response = request(daemon.socket, action: 'run', spec: spec)
            expect(response['exit_code']).to eq(0), "run #{i} should pass; got: #{response.inspect}"
          end
        ensure
          daemon.stop
        end
      end
    end
  end

  describe 'fork mode' do
    it 'runs a passing spec in a forked child and returns exit_code 0' do
      with_temp_project do |project|
        spec = write_passing_spec(project)
        daemon = spawn_daemon(socket_path: temp_socket('fork-pass'), extra_args: ['--fork'], cwd: project)
        begin
          response = request(daemon.socket, action: 'run', spec: spec)
          expect(response['exit_code']).to eq(0)
          expect(response['stdout']).to include('1 example, 0 failures')
        ensure
          daemon.stop
        end
      end
    end

    it 'fork mode does not require --allow-plain-ruby' do
      with_temp_project do |project|
        spec = write_passing_spec(project)
        daemon = spawn_daemon(socket_path: temp_socket('fork-plain'), extra_args: ['--fork'], cwd: project)
        begin
          response = request(daemon.socket, action: 'run', spec: spec)
          expect(response['exit_code']).to eq(0)
        ensure
          daemon.stop
        end
      end
    end

    it 'reaps each forked worker, leaving no zombie children on the daemon' do
      with_temp_project do |project|
        spec = write_passing_spec(project)
        daemon = spawn_daemon(socket_path: temp_socket('fork-reap'), extra_args: ['--fork'], cwd: project)
        begin
          3.times { expect(request(daemon.socket, action: 'run', spec: spec)['exit_code']).to eq(0) }

          children = `ps --ppid #{daemon.pid} -o stat=`.split("\n").map(&:strip)
          zombies = children.select { |stat| stat.start_with?('Z') }
          expect(zombies).to be_empty, "daemon left zombie children: #{children.inspect}"
        ensure
          daemon.stop
        end
      end
    end
  end

  describe 'security: spec path validation' do
    it 'refuses a spec path that resolves outside the project root' do
      with_temp_project do |project|
        outside = File.expand_path('/tmp/transmute-rspec-it-outside-spec.rb')
        File.write(outside, 'puts :outside')
        daemon = spawn_daemon(socket_path: temp_socket('path-outside'), extra_args: ['--allow-plain-ruby'], cwd: project)
        begin
          response = request(daemon.socket, action: 'run', spec: outside)
          expect(response['exit_code']).to eq(2)
          expect(response['stdout']).to include('refusing spec path outside project root')
        ensure
          daemon.stop
          File.delete(outside) if File.exist?(outside)
        end
      end
    end

    it 'refuses a parent-dir traversal' do
      with_temp_project do |project|
        write_passing_spec(project)
        daemon = spawn_daemon(socket_path: temp_socket('path-traversal'), extra_args: ['--allow-plain-ruby'], cwd: project)
        begin
          response = request(daemon.socket, action: 'run', spec: '../../../etc/passwd')
          expect(response['exit_code']).to eq(2)
          expect(response['stdout']).to include('refusing spec path outside project root')
        ensure
          daemon.stop
        end
      end
    end

    it 'treats a run request with a missing or empty spec as not found' do
      with_temp_project do |project|
        daemon = spawn_daemon(socket_path: temp_socket('empty-spec'), extra_args: ['--allow-plain-ruby'], cwd: project)
        begin
          missing = request(daemon.socket, action: 'run')
          empty = request(daemon.socket, action: 'run', spec: '')
          expect(missing['exit_code']).to eq(2)
          expect(empty['exit_code']).to eq(2)
          expect(empty['stdout']).to include('spec file not found')
        ensure
          daemon.stop
        end
      end
    end
  end

  describe 'cli' do
    it 'prints help and exits 0 for --help even without a subcommand' do
      out_path = "#{temp_socket('help')}.out"
      pid = Process.spawn(
        RbConfig.ruby, '-I', DaemonHelpers::LIB_PATH, DaemonHelpers::DAEMON_BIN,
        '--help',
        out: out_path, err: out_path
      )
      _, status = Process.waitpid2(pid)
      out = File.read(out_path)
      File.delete(out_path)
      expect(status.exitstatus).to eq(0)
      expect(out).to include('Usage: transmute-rspec serve')
    end

    it 'prints unknown command and exits 2 for an unrecognized subcommand' do
      out_path = "#{temp_socket('unknown-cmd')}.out"
      pid = Process.spawn(
        RbConfig.ruby, '-I', DaemonHelpers::LIB_PATH, DaemonHelpers::DAEMON_BIN,
        'bogus',
        out: out_path, err: out_path
      )
      _, status = Process.waitpid2(pid)
      out = File.read(out_path)
      File.delete(out_path)
      expect(status.exitstatus).to eq(2)
      expect(out).to include("unknown command 'bogus'")
    end

    it 'refuses to start with a non-positive --read-timeout instead of dropping every client' do
      out_path = "#{temp_socket('bad-timeout')}.out"
      pid = Process.spawn(
        RbConfig.ruby, '-I', DaemonHelpers::LIB_PATH, DaemonHelpers::DAEMON_BIN,
        'serve', '--listen', temp_socket('bad-timeout'), '--read-timeout', '0', '--allow-plain-ruby',
        out: out_path, err: out_path
      )
      _, status = Process.waitpid2(pid)
      out = File.read(out_path)
      File.delete(out_path)
      expect(status.exitstatus).to eq(2)
      expect(out).to include('--read-timeout must be a positive number')
    end
  end

  describe 'security: TCP bind' do
    it 'refuses to start when binding 0.0.0.0 without --allow-public' do
      log_path = "#{temp_socket('tcp-refuse')}.log"
      pid = Process.spawn(
        RbConfig.ruby, '-I', DaemonHelpers::LIB_PATH, DaemonHelpers::DAEMON_BIN,
        'serve', '--listen', 'tcp://0.0.0.0:0', '--allow-plain-ruby',
        out: log_path, err: log_path
      )
      _, status = Process.waitpid2(pid)
      log = File.read(log_path)
      File.delete(log_path)
      expect(status.exitstatus).not_to eq(0)
      expect(log).to include('refusing to bind on 0.0.0.0')
    end
  end

  describe 'TCP transport' do
    it 'assembles a request delivered in fragmented packets and replies over TCP' do
      with_temp_project do |project|
        spec = write_passing_spec(project)
        port = free_tcp_port
        daemon = spawn_daemon(
          socket_path: "tcp://127.0.0.1:#{port}",
          extra_args: ['--allow-plain-ruby'],
          cwd: project
        )
        begin
          response = nil
          Timeout.timeout(8) do
            TCPSocket.open('127.0.0.1', port) do |sock|
              JSON.generate(action: 'run', spec: spec).each_char do |chunk|
                sock.write(chunk)
                sock.flush
              end
              sock.write("\n")
              response = JSON.parse(sock.gets.to_s)
            end
          end
          expect(response['exit_code']).to eq(0)
          expect(response['stdout']).to include('1 example, 0 failures')
        ensure
          daemon.stop
        end
      end
    end
  end

  describe 'security: unix socket permissions' do
    it 'creates the socket with no group/other access even when umask is permissive' do
      with_temp_project do |project|
        sock = temp_socket('umask')
        original_umask = File.umask(0o000)
        begin
          daemon = spawn_daemon(socket_path: sock, extra_args: ['--allow-plain-ruby'], cwd: project)
          mode = File.stat(sock).mode & 0o077
          expect(mode).to eq(0), "expected zero group/other bits, got 0o#{mode.to_s(8)} (full mode 0o#{(File.stat(sock).mode & 0o777).to_s(8)})"
          daemon.stop
        ensure
          File.umask(original_umask)
        end
      end
    end
  end

  describe 'plain-Ruby refusal' do
    it 'refuses to serve in in-process mode without --allow-plain-ruby' do
      with_temp_project do |project|
        log_path = "#{temp_socket('plain-refuse')}.log"
        pid = Process.spawn(
          RbConfig.ruby, '-I', DaemonHelpers::LIB_PATH, DaemonHelpers::DAEMON_BIN,
          'serve', '--listen', temp_socket('plain-refuse'),
          chdir: project, out: log_path, err: log_path
        )
        _, status = Process.waitpid2(pid)
        log = File.read(log_path)
        File.delete(log_path)
        expect(status.exitstatus).not_to eq(0)
        expect(log).to include('refusing to serve in in-process mode without Rails')
      end
    end
  end

  describe 'operational' do
    it 'stats action reports pid, uptime, requests_handled, mode' do
      with_temp_project do |project|
        spec = write_passing_spec(project)
        daemon = spawn_daemon(socket_path: temp_socket('stats'), extra_args: ['--allow-plain-ruby'], cwd: project)
        begin
          request(daemon.socket, action: 'run', spec: spec)
          response = request(daemon.socket, action: 'stats')
          payload = JSON.parse(response['stdout'])
          expect(payload['pid']).to eq(daemon.pid)
          expect(payload['mode']).to eq('in-process')
          expect(payload['requests_handled']).to be >= 1
          expect(payload['uptime_s']).to be >= 0
        ensure
          daemon.stop
        end
      end
    end

    it 'quit action shuts down the daemon cleanly' do
      with_temp_project do |project|
        daemon = spawn_daemon(socket_path: temp_socket('quit'), extra_args: ['--allow-plain-ruby'], cwd: project)
        response = request(daemon.socket, action: 'quit')
        expect(response['stdout']).to eq('bye')
        deadline = Time.now + 5
        exited = false
        until Time.now > deadline
          reaped, _status = Process.waitpid2(daemon.pid, Process::WNOHANG)
          if reaped
            exited = true
            break
          end
          sleep 0.05
        end
        expect(exited).to be(true), 'expected daemon to exit within 5s of quit'
      end
    end

    it 'survives a client that disconnects without reading the response' do
      with_temp_project do |project|
        spec = write_passing_spec(project)
        daemon = spawn_daemon(socket_path: temp_socket('disconnect'), extra_args: ['--allow-plain-ruby'], cwd: project)
        begin
          rude_client = UNIXSocket.open(daemon.socket)
          rude_client.puts(JSON.generate(action: 'run', spec: spec))
          rude_client.close

          sleep 0.5

          response = request(daemon.socket, action: 'ping')
          expect(response['exit_code']).to eq(0)
          expect(response['stdout']).to eq('pong')
        ensure
          daemon.stop
        end
      end
    end

    it 'does not wedge when a client connects and sends nothing' do
      with_temp_project do |project|
        daemon = spawn_daemon(
          socket_path: temp_socket('connect-hold'),
          extra_args: ['--allow-plain-ruby', '--read-timeout', '1'],
          cwd: project
        )
        begin
          mute_client = UNIXSocket.open(daemon.socket)

          response = nil
          Timeout.timeout(8) do
            response = request(daemon.socket, action: 'ping')
          end
          expect(response['exit_code']).to eq(0)
          expect(response['stdout']).to eq('pong')

          expect(daemon.log_contents).to include('read timeout')
        ensure
          mute_client.close if mute_client && !mute_client.closed?
          daemon.stop
        end
      end
    end

    it 'drops a client that floods a newline-less line yet still answers a later ping' do
      with_temp_project do |project|
        daemon = spawn_daemon(socket_path: temp_socket('flood'), extra_args: ['--allow-plain-ruby'], cwd: project)
        begin
          flood_client = UNIXSocket.open(daemon.socket)
          flood_client.write('x' * (128 * 1024)) # > MAX_REQUEST_LINE_BYTES, no newline
          flood_client.flush

          response = nil
          Timeout.timeout(8) do
            response = request(daemon.socket, action: 'ping')
          end
          expect(response['exit_code']).to eq(0)
          expect(response['stdout']).to eq('pong')

          expect(daemon.log_contents).to include('request line exceeded')
        ensure
          flood_client.close if flood_client && !flood_client.closed?
          daemon.stop
        end
      end
    end
  end
end
