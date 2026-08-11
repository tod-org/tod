# Task: Project update, archive, and unarchive API endpoints

Add Todoist API client functions for updating, archiving, and unarchiving projects, along with corresponding CLI subcommands. The Todoist API supports `POST /projects/{id}` (with `name`, `color`, `is_favorite`, `view_style` body fields), `POST /projects/{id}/archive`, and `POST /projects/{id}/unarchive`. Currently only create, list, and delete project operations exist. Also consider exposing an archived projects listing endpoint (`GET /projects/archived`).

Part of #1760 (API feature parity).
