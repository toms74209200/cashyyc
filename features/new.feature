Feature: cyyc new

  Scenario: Apply template options and keep the features the template declares
    Given no devcontainer config exists
    And the "java" template is selected
    And every template option takes its default
    And the "git" feature is selected
    When running "cyyc new"
    Then .devcontainer/devcontainer.json is created with a valid template
    And no template option placeholder remains in .devcontainer/devcontainer.json
    And .devcontainer/devcontainer.json declares the feature "ghcr.io/devcontainers/features/java:1"
    And .devcontainer/devcontainer.json declares the feature "ghcr.io/devcontainers/features/git:1"
    And the command exits successfully

  Scenario: Write the Dockerfile a dockerfile-type template ships
    Given no devcontainer config exists
    And the "cpp" template is selected
    And every template option takes its default
    And no feature is selected
    When running "cyyc new"
    Then .devcontainer/Dockerfile is created
    And no template option placeholder remains in .devcontainer/Dockerfile
    And .github/dependabot.yml is not created
    And the command exits successfully

  Scenario: Write the compose file a dockerCompose-type template ships
    Given no devcontainer config exists
    And the "postgres" template is selected
    And every template option takes its default
    And no feature is selected
    When running "cyyc new"
    Then .devcontainer/docker-compose.yml is created
    And the command exits successfully

  Scenario: Keep the mode and the nesting of the files a template ships
    Given no devcontainer config exists
    And the "kubernetes" template is selected
    And every template option takes its default
    And no feature is selected
    When running "cyyc new"
    Then .devcontainer/local-features/copy-kube-config/install.sh is created
    And .devcontainer/ensure-mount-sources is executable
    And the command exits successfully

  Scenario: Fail when devcontainer.json already exists
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    When running "cyyc new"
    Then the command exits with a non-zero status

  Scenario: Fail without writing anything when a file the template ships is already present
    Given no devcontainer config exists
    And the file ".devcontainer/Dockerfile" already exists
    And the "cpp" template is selected
    When running "cyyc new"
    Then the command exits with a non-zero status
    And .devcontainer/devcontainer.json is not created
