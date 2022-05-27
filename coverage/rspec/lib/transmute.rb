# frozen_string_literal: true

require 'coverage'
require 'json'
require 'singleton'

Coverage.start

class Transmute
  @instance = new

  private_class_method :new

  attr_reader :data, :previous_result

  class << self
    attr_reader :instance

    def start
      instance.start
    end

    def add_coverage(example)
      self.instance.add_coverage(example)
    end

    def store!
      self.instance.store!
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
    current_result.map do |file, lines|
      lines.map.with_index do |value, line|
        next if value.nil? || value.zero?

        @data["#{file}:#{line + 1}"] ||= []
        @data["#{file}:#{line + 1}"] << test_path
        @data["#{file}:#{line + 1}"].uniq!
      end
    end
    @previous_result = current_result
  end

  def store!
    File.write('transmute.json', JSON.pretty_generate(data))
  end

  private

  def select_project_files(result = Coverage.peek_result)
    result.select do |file_path, _lines|
      file_path.start_with?(Dir.pwd) && !file_path.start_with?(Dir.pwd + '/spec')
    end
  end

  def diff_result(current_result)
    previous_result.merge(current_result) do |_file_path, previous_line, current_line|
      previous_line.zip(current_line).map { |values| values[0] == values[1] ? nil : values[1].to_i - values[0].to_i }
    end
  end
end
