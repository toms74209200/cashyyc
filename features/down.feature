Feature: cyyc down

  Scenario: Down a running Single-config container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for this config
    When running "cyyc down"
    Then the container is removed

  Scenario: Down a running Compose container
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for this config
    When running "cyyc down"
    Then the container is removed

  Scenario: Down a stopped container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a stopped container exists for this config
    When running "cyyc down"
    Then the container is removed

  Scenario: Down when no container exists
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And no container exists for this config
    When running "cyyc down"
    Then the command exits successfully

  Scenario: Down a named environment
    Given a "python" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for this config
    When running "cyyc down python"
    Then the container is removed

  Scenario: Multiple environments exist but no name is given
    Given a "python" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a "rust" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    When running "cyyc down"
    Then the available environment names are printed
    And the command exits with a non-zero status

  Scenario: No devcontainer config exists
    Given no devcontainer config exists
    When running "cyyc down"
    Then the command exits with a non-zero status
