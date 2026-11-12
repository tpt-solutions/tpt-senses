# Security Policy

## Supported versions

`tpt-senses` is pre-release. Only the latest `0.x` release line receives security fixes.

## Reporting a vulnerability

Please **do not** open a public issue for security problems.

Report vulnerabilities privately to TPT Solutions through the security contact published on the
project's repository, or by using GitHub's private vulnerability reporting for this repository. Include:

- a description of the issue and its impact
- steps or a minimal input to reproduce it
- the affected version or commit

You should receive an acknowledgement within 5 working days. We will coordinate a fix and
disclosure timeline with you.

## Scope notes

Sensor data may come from untrusted sources (network devices, remote robots, third-party APIs).
Observations produced from such data must not be treated as facts; see spec §33. Reports about
trust metadata, provenance spoofing, or unchecked input that causes panics or unbounded memory use
are in scope.
