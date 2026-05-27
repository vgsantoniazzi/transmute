require_relative "./spec_helper"

RSpec.describe User do
  context "defaults" do
    it "github defaults to 'vgsantoniazzi'" do
      expect(User.new.github).to eq("vgsantoniazzi")
    end

    it "repos defaults to 0" do
      expect(User.new.repos).to eq(0)
    end

    it "default user is not admin" do
      expect(User.new.admin?).to eq(false)
    end

    it "default user is not pro" do
      expect(User.new.pro?).to eq(false)
    end
  end

  context "constructor arguments" do
    it "accepts a custom github handle" do
      expect(User.new("test").github).to eq("test")
    end

    it "accepts a custom repos count" do
      expect(User.new("test", 7).repos).to eq(7)
    end
  end

  context "#pro?" do
    it "false when repos == 0" do
      expect(User.new("x", 0).pro?).to eq(false)
    end

    it "false when repos == 1" do
      expect(User.new("x", 1).pro?).to eq(false)
    end

    it "false at the boundary repos == 10" do
      expect(User.new("x", 10).pro?).to eq(false)
    end

    it "true just past the boundary repos == 11" do
      expect(User.new("x", 11).pro?).to eq(true)
    end

    it "true when repos == 20" do
      expect(User.new("x", 20).pro?).to eq(true)
    end
  end

  context "#admin?" do
    it "true when github == 'admin'" do
      expect(User.new("admin", 1).admin?).to eq(true)
    end

    it "false when github != 'admin'" do
      expect(User.new("test", 1).admin?).to eq(false)
    end

    it "false when github is a prefix of 'admin'" do
      expect(User.new("admi", 1).admin?).to eq(false)
    end

    it "false when github has a suffix on 'admin'" do
      expect(User.new("admins", 1).admin?).to eq(false)
    end
  end
end
