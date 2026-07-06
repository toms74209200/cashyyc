Feature: cyyc new

  Scenario: Create devcontainer.json with template and no features
    Given no devcontainer config exists
    When running "cyyc new" and selecting the first template and confirming features
    Then .devcontainer/devcontainer.json is created with a valid template
    And the command exits successfully

  Scenario: Fail when devcontainer.json already exists
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    When running "cyyc new"
    Then the command exits with a non-zero status
