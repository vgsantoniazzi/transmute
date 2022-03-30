require_relative "../../../../coverage/rspec/transmute"
require "rspec"

RSpec.configure do |config|
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
