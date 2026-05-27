# frozen_string_literal: true

require 'coverage'
require 'singleton'
require 'sqlite3'

Coverage.start

class Transmute
  SCHEMA_VERSION = '1'
  DEFAULT_PATH = 'transmute.sqlite'

  SCHEMA_DDL = <<~SQL
    CREATE TABLE IF NOT EXISTS schema_meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS files (
        id   INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE
    );

    CREATE TABLE IF NOT EXISTS specs (
        id   INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE
    );

    CREATE TABLE IF NOT EXISTS coverage (
        file_id INTEGER NOT NULL,
        line    INTEGER NOT NULL,
        spec_id INTEGER NOT NULL,
        PRIMARY KEY (file_id, line, spec_id)
    ) WITHOUT ROWID;

    CREATE INDEX IF NOT EXISTS idx_coverage_spec ON coverage(spec_id);
  SQL

  @instance = new

  private_class_method :new

  attr_reader :data, :previous_result

  class << self
    attr_reader :instance

    def start
      instance.start
    end

    def add_coverage(example)
      instance.add_coverage(example)
    end

    def store!(path = DEFAULT_PATH)
      instance.store!(path)
    end
  end

  def start
    @data = {}
    @previous_result = {}
  end

  def add_coverage(example)
    result = select_project_files
    test_path = example.metadata[:file_path]
    process(test_path, result)
  end

  def process(test_path, result)
    current_result = diff_result(result)
    current_result.each do |file, lines|
      lines.each_with_index do |value, index|
        next if value.nil? || value.zero?

        key = [file, index + 1]
        @data[key] ||= []
        @data[key] << test_path unless @data[key].include?(test_path)
      end
    end
    @previous_result = current_result
  end

  def store!(path = DEFAULT_PATH)
    File.delete(path) if File.exist?(path)
    db = SQLite3::Database.new(path)
    begin
      db.execute_batch(SCHEMA_DDL)
      db.execute(
        "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('version', ?)",
        [SCHEMA_VERSION]
      )
      write_entries(db)
    ensure
      db.close
    end
  end

  private

  def write_entries(db)
    file_ids = {}
    spec_ids = {}
    db.transaction do
      @data.each do |(file, line), specs|
        file_ids[file] ||= upsert_path_id(db, 'files', file)
        specs.each do |spec|
          spec_ids[spec] ||= upsert_path_id(db, 'specs', spec)
          db.execute(
            'INSERT OR IGNORE INTO coverage (file_id, line, spec_id) VALUES (?, ?, ?)',
            [file_ids[file], line, spec_ids[spec]]
          )
        end
      end
    end
  end

  def upsert_path_id(db, table, path)
    raise ArgumentError, "unknown table #{table}" unless %w[files specs].include?(table)

    db.execute("INSERT OR IGNORE INTO #{table} (path) VALUES (?)", [path])
    db.get_first_value("SELECT id FROM #{table} WHERE path = ?", [path])
  end

  def select_project_files(result = Coverage.peek_result)
    result.select do |file_path, _lines|
      file_path.start_with?(Dir.pwd) && !file_path.start_with?("#{Dir.pwd}/spec")
    end
  end

  def diff_result(current_result)
    previous_result.merge(current_result) do |_file_path, previous_line, current_line|
      previous_line.zip(current_line).map do |values|
        values[0] == values[1] ? nil : values[1].to_i - values[0].to_i
      end
    end
  end
end
