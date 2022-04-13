# Transmute

Ruby gem to help you to generate the reverse-coverage relationship between code and tests. The output is a `.transmute.json` file with key as source code + line and the value is an array with all specs that touch this particular line.

## Installation

Add this line to your application's Gemfile:

```ruby
gem "transmute"
```

And then execute:

    $ bundle install

Or install it yourself as:

    $ gem install transmute

## Usage

```ruby
require "rspec"
require "transmute"

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
```

## Test

We prioritize end-to-end tests, so you'll not see spec file here. In order to make sure that everything is working properly, you should add rust specs in the engine folder.

## License

The gem is available as open source under the terms of the [MIT License](https://opensource.org/licenses/MIT).