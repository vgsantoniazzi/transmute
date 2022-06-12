require_relative "./spec_helper"

RSpec.describe App do
  context "properly set the name" do
    it "use default handle" do
      expect(App.new.attr[:name]).to eq("transmute::main")
    end

    it "when provided handle" do
      expect(App.new("transmute-ruby").attr[:name]).to eq("transmute-ruby")
    end
  end
end
