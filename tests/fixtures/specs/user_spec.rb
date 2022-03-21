require_relative "../app/user"

RSpec.describe User do
  context "properly set the github handle" do
    it "use default handle" do
      expect(User.new.github).to eq("vgsantoniazzi")
    end

    it "when provided handle" do
      expect(User.new("test").github).to eq("test")
    end

    it "default not pro if not provided" do
      expect(User.new("test").pro?).to eq(false)
    end

    it "when pro user" do
      expect(User.new("test", 20).pro?).to eq(true)
    end

    it "when normal user" do
      expect(User.new("test", 1).pro?).to eq(false)
    end

    it "when admin" do
      expect(User.new("admin", 1).admin?).to eq(true)
    end

    it "when not admin" do
      expect(User.new("test", 1).admin?).to eq(false)
    end
  end
end
