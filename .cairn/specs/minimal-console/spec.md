# Minimal Console Specification

## Purpose

Defines the `minimal-console` capability.

## Requirements

### Requirement: Minimal actionable live console

r[kiln.web.minimal_console]
The live console MUST prioritize readable retained execution observations and direct inspection over decorative chrome. All existing work and CI routes, counts, authentication, confirmations, appearance, density and keyboard contracts MUST remain available. Changes MUST NOT fabricate observations, introduce external assets or modify backend authority.

#### Scenario: Overview to exact failure output

- GIVEN retained failed, successful, active, queued, unavailable or unknown project observations
- WHEN the operator opens Overview
- THEN counts MUST remain explicit and actionable, latest project executions MUST be compact and attention-first, and a Logs link MUST open that exact execution without stale filters or another run being substituted

#### Scenario: Progressive disclosure and restored routes

- GIVEN a secondary navigation URL or non-default exact-source/order run filters
- WHEN the workspace restores the URL
- THEN the selected navigation group and active advanced filters MUST be exposed, every underlying route/control MUST remain reachable, and explicit execution confirmations MUST remain unchanged

#### Scenario: Mobile and keyboard

- GIVEN a 320px viewport or keyboard navigation
- WHEN the operator uses status links, project rows, disclosures, issue capture or search
- THEN the surface MUST avoid document-level horizontal overflow, preserve visible focus and mobile touch targets, and display unknown timing as unknown rather than zero
