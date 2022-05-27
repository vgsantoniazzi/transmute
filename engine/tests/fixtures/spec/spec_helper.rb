require_relative "../../../../coverage/rspec/lib/transmute"
require_relative "../app/user"
require_relative "../app/app"
require "rspec"

RSpec.configure do |config|
  config.filter_run_excluding broken: true

  config.before(:suite) do
    Transmute.start if ENV["COVERAGE"]
  end

  config.around do |example|
    example.run
    Transmute.add_coverage(example) if ENV["COVERAGE"]
  end

  config.after(:suite) do
    Transmute.store! if ENV["COVERAGE"]
  end
end
