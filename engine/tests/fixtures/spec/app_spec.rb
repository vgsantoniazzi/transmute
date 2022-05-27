require_relative "./spec_helper"

RSpec.describe App do
  context "properly set the name" do
    it "use default handle" do
      expect(App.new.name).to eq("transmute")
    end

    it "when provided handle" do
      expect(User.new("transmute-ruby").github).to eq("transmute-ruby")
    end
  end
end
