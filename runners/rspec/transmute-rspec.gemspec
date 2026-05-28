# frozen_string_literal: true

require_relative 'lib/transmute/rspec/version'

Gem::Specification.new do |spec|
  spec.name          = 'transmute-rspec'
  spec.version       = Transmute::Rspec::VERSION
  spec.authors       = ['Victor Antoniazzi']
  spec.email         = ['vgsantoniazzi@gmail.com']

  spec.summary       = 'Persistent RSpec runner daemon for transmute mutation testing.'
  spec.description   = 'A long-running daemon that boots Rails + RSpec once and accepts spec-run requests over a Unix socket. Replaces per-mutation `bundle exec rspec` invocations with sub-second in-process runs. Used by the transmute mutation testing engine.'
  spec.homepage      = 'https://github.com/vgsantoniazzi/transmute'
  spec.license       = 'GPL-3.0'
  spec.required_ruby_version = Gem::Requirement.new('>= 3.0.0')

  spec.metadata['homepage_uri'] = spec.homepage
  spec.metadata['source_code_uri'] = 'https://github.com/vgsantoniazzi/transmute'

  spec.files = Dir.chdir(File.expand_path(__dir__)) do
    Dir.glob('lib/**/*.rb') + Dir.glob('exe/*') + ['README.md', 'transmute-rspec.gemspec']
  end
  spec.bindir = 'exe'
  spec.executables = %w[transmute-rspec transmute-rspec-run]
  spec.require_paths = ['lib']

  spec.add_runtime_dependency 'rspec-core', '~> 3.0'

  spec.add_development_dependency 'rspec', '~> 3.0'
end
