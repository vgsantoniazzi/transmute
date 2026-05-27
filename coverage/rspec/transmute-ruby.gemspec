# frozen_string_literal: true

require_relative 'lib/transmute/version'

Gem::Specification.new do |spec|
  spec.name          = 'transmute-ruby'
  spec.version       = Transmute::VERSION
  spec.authors       = ['Victor Antoniazzi']
  spec.email         = ['vgsantoniazzi@gmail.com']

  spec.summary       = 'Generate the reverse relationship between code and specs.'
  spec.description   = 'Ruby gem to help you to generate the reverse-coverage relationship between code and tests. The output is a `transmute.sqlite` database that maps every source line to the specs that touch it, used by the transmute engine for targeted mutation testing.'
  spec.homepage      = 'https://github.com/vgsantoniazzi/transmute'
  spec.license       = 'GPL-3.0'
  spec.required_ruby_version = Gem::Requirement.new('>= 2.4.0')

  spec.metadata['homepage_uri'] = spec.homepage
  spec.metadata['source_code_uri'] = 'https://github.com/vgsantoniazzi/transmute'

  spec.files = Dir.chdir(File.expand_path(__dir__)) do
    Dir.glob('lib/**/*.rb') + ['README.md', 'transmute-ruby.gemspec']
  end

  spec.require_paths = ['lib']

  spec.add_runtime_dependency 'sqlite3', '~> 1.4'
end
