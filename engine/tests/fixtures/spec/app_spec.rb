require_relative "./spec_helper"

RSpec.describe App do
  context "defaults" do
    it "name defaults to 'transmute::main'" do
      expect(App.new.attr[:name]).to eq("transmute::main")
    end

    it "attr is a Hash" do
      expect(App.new.attr).to be_a(Hash)
    end

    it "attr exposes only the :name key on a fresh instance" do
      expect(App.new.attr.keys).to eq([:name])
    end
  end

  context "constructor arguments" do
    it "accepts a custom name" do
      expect(App.new("transmute-ruby").attr[:name]).to eq("transmute-ruby")
    end

    it "accepts an empty name" do
      expect(App.new("").attr[:name]).to eq("")
    end
  end

  context "attr writer" do
    it "writes and reads back the attr hash" do
      app = App.new
      app.attr = { name: "replaced" }
      expect(app.attr[:name]).to eq("replaced")
    end

    it "writes do not bleed across instances" do
      a = App.new("a")
      b = App.new("b")
      a.attr = { name: "mutated" }
      expect(b.attr[:name]).to eq("b")
    end
  end
end
