class User
  attr_accessor :github, :repos
  def initialize(github = "vgsantoniazzi", repos = 0)
    @github = github
    @repos = repos
  end

  def pro?
    return false if repos <= 10
    true
  end

  def admin?
    github == "admin"
  end
end
