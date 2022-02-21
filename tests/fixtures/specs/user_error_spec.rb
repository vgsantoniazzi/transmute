require_relative "../app/user"

RSpec.describe User do
  context "properly set the github handle" do
    it "use default" do
      expect(User.new.github).to eq("vgsantoniazz")
    end
  end
end
