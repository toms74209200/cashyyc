Feature: cyyc new

  Scenario: Apply template options and keep the features the template declares
    Given no devcontainer config exists
    When running "cyyc new" and selecting the "java" template with the default options and the "git" feature
    Then .devcontainer/devcontainer.json is created with a valid template
    And no template option placeholder remains in .devcontainer/devcontainer.json
    And .devcontainer/devcontainer.json declares the feature "ghcr.io/devcontainers/features/java:1"
    And .devcontainer/devcontainer.json declares the feature "ghcr.io/devcontainers/features/git:1"
    And the command exits successfully

  Scenario: Fail when devcontainer.json already exists
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    When running "cyyc new"
    Then the command exits with a non-zero status
