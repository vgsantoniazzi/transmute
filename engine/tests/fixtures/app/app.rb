class App < Object
  attr_accessor :attr
  def initialize(name = "transmute::main")
    @attr ||= {}
    @attr[:name] = name
  end
end
