# frozen_string_literal: true

require 'spec_helper'
require 'sqlite3'

RSpec.describe Transmute::Recorder do
  subject(:recorder) { described_class.new.tap(&:start) }

  describe '#process' do
    it 'records the spec that touched each executed line on the first snapshot' do
      result = { 'app/user.rb' => [1, 0, 2, nil] }

      recorder.process('spec/user_spec.rb', result)

      expect(recorder.data).to eq(
        ['app/user.rb', 1] => ['spec/user_spec.rb'],
        ['app/user.rb', 3] => ['spec/user_spec.rb']
      )
    end

    it 'attributes only the lines a spec changed, not lines unchanged since the last snapshot' do
      recorder.process('spec/a_spec.rb', 'app/user.rb' => [1, 1])
      recorder.process('spec/b_spec.rb', 'app/user.rb' => [1, 2])

      expect(recorder.data).to eq(
        ['app/user.rb', 1] => ['spec/a_spec.rb'],
        ['app/user.rb', 2] => %w[spec/a_spec.rb spec/b_spec.rb]
      )
    end

    it 'does not duplicate a spec already recorded for a line' do
      recorder.process('spec/user_spec.rb', 'app/user.rb' => [1])
      recorder.process('spec/user_spec.rb', 'app/user.rb' => [2])

      expect(recorder.data).to eq(['app/user.rb', 1] => ['spec/user_spec.rb'])
    end
  end

  describe '#store!' do
    around do |example|
      Dir.mktmpdir('transmute-recorder') do |dir|
        original = Dir.pwd
        Dir.chdir(dir)
        begin
          example.run
        ensure
          Dir.chdir(original)
        end
      end
    end

    it 'writes a sqlite database that maps source lines to covering specs' do
      recorder.process('spec/user_spec.rb', 'app/user.rb' => [1, 0, 1])

      path = recorder.store!('transmute.sqlite')

      db = SQLite3::Database.new(path)
      begin
        rows = db.execute(<<~SQL)
          SELECT files.path, coverage.line, specs.path
          FROM coverage
          JOIN files ON files.id = coverage.file_id
          JOIN specs ON specs.id = coverage.spec_id
          ORDER BY coverage.line
        SQL
        expect(rows).to eq([
          ['app/user.rb', 1, 'spec/user_spec.rb'],
          ['app/user.rb', 3, 'spec/user_spec.rb']
        ])
      ensure
        db.close
      end
    end

    it 'records the schema version and capture-time cwd in schema_meta' do
      recorder.process('spec/user_spec.rb', 'app/user.rb' => [1])

      path = recorder.store!('transmute.sqlite')

      db = SQLite3::Database.new(path)
      begin
        meta = db.execute('SELECT key, value FROM schema_meta').to_h
        expect(meta['version']).to eq(described_class::SCHEMA_VERSION)
        expect(meta['cwd']).to eq(Dir.pwd)
      ensure
        db.close
      end
    end

    it 'writes atomically, leaving no .tmp file behind' do
      recorder.process('spec/user_spec.rb', 'app/user.rb' => [1])

      recorder.store!('transmute.sqlite')

      expect(Dir.glob('transmute.sqlite.tmp.*')).to be_empty
    end

    context 'when TEST_ENV_NUMBER is set' do
      around do |example|
        original = ENV['TEST_ENV_NUMBER']
        ENV['TEST_ENV_NUMBER'] = '2'
        begin
          example.run
        ensure
          ENV['TEST_ENV_NUMBER'] = original
        end
      end

      it 'scopes the output path to the worker number' do
        recorder.process('spec/user_spec.rb', 'app/user.rb' => [1])

        path = recorder.store!('transmute.sqlite')

        expect(path).to eq('transmute.2.sqlite')
        expect(File.exist?('transmute.2.sqlite')).to be(true)
      end
    end
  end
end
