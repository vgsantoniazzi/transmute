require_relative "../app/user"

RSpec.describe User do
  context "properly set the github handle" do
    it "use default" do
      expect(User.new.github).to eq("vgsantoniazzi")
    end

    it "when provided" do
      expect(User.new("test").github).to eq("test")
    end
  end
end
