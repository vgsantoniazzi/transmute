# transmute-ruby

Ruby gem that produces the reverse-coverage relationship between code and tests. Writes a `transmute.sqlite` database mapping every source line to the specs that touch it, consumed by the [transmute](https://github.com/vgsantoniazzi/transmute) mutation testing engine.

## Installation

`transmute-ruby` 0.3.1 isn't on RubyGems yet (the published 0.1.0 predates the SQLite format), so install it from the repository:

```ruby
gem 'transmute-ruby', github: 'vgsantoniazzi/transmute', glob: 'coverage/rspec/*.gemspec'
```

The gem depends on the `sqlite3` gem, which requires `libsqlite3-dev` headers on the build host.

Then run `bundle install`.

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

Unit specs for the recorder live in `spec/`; run them with `bundle exec rspec`. End-to-end coverage of the full pipeline lives in the engine's Rust integration tests in `engine/`.

## License

GPL-3.0. See [LICENSE](../../LICENSE).
