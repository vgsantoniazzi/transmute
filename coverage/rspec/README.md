# Transmute

Ruby gem that produces the reverse-coverage relationship between code and tests. Writes a `transmute.sqlite` database mapping every source line to the specs that touch it, consumed by the [transmute](https://github.com/vgsantoniazzi/transmute) mutation testing engine.

## Installation

Add this line to your application's Gemfile:

```ruby
gem "transmute-ruby"
```

The gem depends on the `sqlite3` gem, which requires `libsqlite3-dev` headers on the build host.

And then execute:

    $ bundle install

Or install it yourself as:

    $ gem install transmute-ruby

## Usage

```ruby
require "rspec"
require "transmute"

RSpec.configure do |config|
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
```

`Transmute.store!` writes to `transmute.sqlite` by default; pass a path to override.

## Test

We prioritize end-to-end tests, so you'll not see spec files here. To verify everything works, run the engine's Rust integration tests in `engine/`.

## License

The gem is available as open source under the terms of the [MIT License](https://opensource.org/licenses/MIT).
