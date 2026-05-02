Feature: cyyc shell

  Scenario: Start a fresh container from an Image config
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running

  Scenario: Start a fresh container from an Image config with a feature
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has feature "ghcr.io/devcontainers/features/node:1"
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running
    And the command "node --version" succeeds in the resulting shell

  Scenario: Start a fresh container from a Dockerfile config
    Given a devcontainer config with Dockerfile:
      """
      FROM mcr.microsoft.com/devcontainers/base:debian
      """
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running

  Scenario: Start a fresh container from a Dockerfile config with a feature
    Given a devcontainer config with Dockerfile:
      """
      FROM mcr.microsoft.com/devcontainers/base:debian
      """
    And the config has feature "ghcr.io/devcontainers/features/node:1"
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running
    And the command "node --version" succeeds in the resulting shell

  Scenario: Start a fresh container from a DockerfileBuild config
    Given a devcontainer config with build using Dockerfile:
      """
      FROM mcr.microsoft.com/devcontainers/base:debian
      """
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running

  Scenario: Start a fresh container from a DockerfileBuild config with a feature
    Given a devcontainer config with build using Dockerfile:
      """
      FROM mcr.microsoft.com/devcontainers/base:debian
      """
    And the config has feature "ghcr.io/devcontainers/features/node:1"
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running
    And the command "node --version" succeeds in the resulting shell

  Scenario: Start a fresh container from a Compose config
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running

  Scenario: Start a fresh container from a Compose config with a feature
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And the config has feature "ghcr.io/devcontainers/features/node:1"
    And no container exists for this config
    When running "cyyc shell"
    Then the container is running
    And the command "node --version" succeeds in the resulting shell

  Scenario: Restart a stopped Single-type container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a stopped container exists for this config
    When running "cyyc shell"
    Then the container is running
    And the existing container is reused

  Scenario: Restart a stopped Compose container
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And a stopped container exists for this config
    When running "cyyc shell"
    Then the container is running
    And the existing container is reused

  Scenario: Open a second session into a running Image-config container
    Given a devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for this config
    When running "cyyc shell"
    Then a new shell session is opened in the existing container

  Scenario: Open a second session into a running Dockerfile-config container
    Given a devcontainer config with Dockerfile:
      """
      FROM mcr.microsoft.com/devcontainers/base:debian
      """
    And a running container exists for this config
    When running "cyyc shell"
    Then a new shell session is opened in the existing container

  Scenario: Open a second session into a running DockerfileBuild-config container
    Given a devcontainer config with build using Dockerfile:
      """
      FROM mcr.microsoft.com/devcontainers/base:debian
      """
    And a running container exists for this config
    When running "cyyc shell"
    Then a new shell session is opened in the existing container

  Scenario: Open a second session into a running Compose container
    Given a devcontainer config using docker-compose service "app" with image "mcr.microsoft.com/devcontainers/base:debian"
    And a running container exists for this config
    When running "cyyc shell"
    Then a new shell session is opened in the existing container

  Scenario: Select a named environment
    Given a "python" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And no container exists for this config
    When running "cyyc shell python"
    Then the container is running

  Scenario: Multiple environments exist but no name is given
    Given a "python" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And a "rust" devcontainer config with image "mcr.microsoft.com/devcontainers/base:debian"
    And no container exists for this config
    When running "cyyc shell"
    Then the available environment names are printed
    And the command exits with a non-zero status

  Scenario: No devcontainer config exists
    Given no devcontainer config exists
    When running "cyyc shell"
    Then the command exits with a non-zero status
