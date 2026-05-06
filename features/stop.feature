Feature: cyyc stop

  Scenario: Stop a running Single-config container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for this config
    When running "cyyc stop"
    Then the container is stopped
    And the container is not removed

  Scenario: Stop a running Compose container
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for this config
    When running "cyyc stop"
    Then the container is stopped
    And the container is not removed

  Scenario: Stop when container is already stopped
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a stopped container exists for this config
    When running "cyyc stop"
    Then the command exits successfully
    And the container is not removed

  Scenario: Stop when no container exists
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And no container exists for this config
    When running "cyyc stop"
    Then the command exits successfully

  Scenario: Stop a named environment
    Given a "python" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for this config
    When running "cyyc stop python"
    Then the container is stopped

  Scenario: Multiple environments exist but no name is given
    Given a "python" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a "rust" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    When running "cyyc stop"
    Then the available environment names are printed
    And the command exits with a non-zero status

  Scenario: No devcontainer config exists
    Given no devcontainer config exists
    When running "cyyc stop"
    Then the command exits with a non-zero status
