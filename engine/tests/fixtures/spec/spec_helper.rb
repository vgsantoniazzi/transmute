require_relative "../../../../coverage/rspec/lib/transmute"
require_relative "../app/user"
require "rspec"

RSpec.configure do |config|
  config.filter_run_excluding broken: true

  config.before(:suite) do
    Transmute.instance.start
  end

  config.around do |example|
    example.run
    Transmute.instance.add_coverage(example)
  end

  config.after(:suite) do
    Transmute.instance.store!
  end
end
