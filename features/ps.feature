Feature: cyyc ps

  Scenario: List with no container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And no container exists for this config
    When running "cyyc ps"
    Then the listing shows the config with status "none"

  Scenario: List with a stopped container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a stopped container exists for this config
    When running "cyyc ps"
    Then the listing shows the config with status "stopped"

  Scenario: List with a running container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for this config
    When running "cyyc ps"
    Then the listing shows the config with status "running"
    And the container ID is printed

  Scenario: List multiple named configs
    Given a "python" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a "rust" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for the "python" config
    And no container exists for the "rust" config
    When running "cyyc ps"
    Then the listing shows "python" with status "running"
    And the listing shows "rust" with status "none"

  Scenario: List a stopped Compose container
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And a stopped container exists for this config
    When running "cyyc ps"
    Then the listing shows the config with status "stopped"

  Scenario: List a Compose config
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And the compose file also defines a runService "db"
    And a running container exists for this config
    When running "cyyc ps"
    Then the listing shows the config with status "running"
    And the "db" service is not listed

  Scenario: No devcontainer config exists
    Given no devcontainer config exists
    When running "cyyc ps"
    Then the command exits with a non-zero status
