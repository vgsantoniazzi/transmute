# frozen_string_literal: true

require_relative 'lib/transmute/version'

Gem::Specification.new do |spec|
  spec.name          = 'transmute-ruby'
  spec.version       = Transmute::VERSION
  spec.authors       = ['Victor Antoniazzi']
  spec.email         = ['vgsantoniazzi@gmail.com']

  spec.summary       = 'Generate reverse relationsip between code and specs.'
  spec.description   = 'Ruby gem to help you to generate the reverse-coverage relationship between code and tests. The output is a `.transmute.json` file with key as source code + line and the value is an array with all specs that touch this particular line.'
  spec.homepage      = 'https://github.com/vgsantoniazzi/transmute'
  spec.license       = 'GPL-3.0'
  spec.required_ruby_version = Gem::Requirement.new('>= 2.4.0')

  spec.metadata['homepage_uri'] = spec.homepage
  spec.metadata['source_code_uri'] = 'https://github.com/vgsantoniazzi/transmute'

  spec.files = Dir.chdir(File.expand_path(__dir__)) do
    Dir.glob('lib/**/*.rb') + ['README.md', 'transmute-ruby.gemspec']
  end

  spec.require_paths = ['lib']
end
