# Use A Global Session Registry With Project-Local Artifacts

Neowright Sessions must be discoverable and controllable from any working directory, so active Session metadata will live in a global Session Registry. Artifacts such as snapshots and logs will remain project-local under the `.neowright/` directory associated with the working directory where the Session was opened, preserving useful debugging context without making Session targeting depend on cwd.
