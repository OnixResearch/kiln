"""Pure admission for the real OnixOS pilot's preparation boundary."""

CONSUMER_ARTIFACTS = frozenset({"home-activation", "activation", "home-files"})


# r[impl kiln.onixos_pilot.closure.preparation]
def preparation_plan(jobs):
    if (len(jobs) != len(CONSUMER_ARTIFACTS)
            or {job.get("attr") for job in jobs} != CONSUMER_ARTIFACTS
            or any("error" in job for job in jobs)):
        raise ValueError("preparation requires exactly the three real consumer artifacts")
    artifacts = {job["attr"]: job for job in jobs}
    if any(set(job["outputs"]) != {"out"} for job in jobs):
        raise ValueError("consumer artifacts require exact out outputs")
    derivations = {name: job["drvPath"] for name, job in artifacts.items()}
    outputs = {name: job["outputs"]["out"] for name, job in artifacts.items()}
    if (len(set(derivations.values())) != len(CONSUMER_ARTIFACTS)
            or len(set(outputs.values())) != len(CONSUMER_ARTIFACTS)):
        raise ValueError("consumer artifacts must have distinct identities")
    if (derivations["activation"] not in artifacts["home-activation"]["inputDrvs"]
            or derivations["home-files"] not in artifacts["activation"]["inputDrvs"]):
        raise ValueError("consumer artifacts do not form the real launcher closure")
    dependencies = {dependency for job in jobs for dependency in job["inputDrvs"]}
    return {"consumer_derivations": derivations, "consumer_outputs": outputs,
            "dependency_derivations": sorted(dependencies - set(derivations.values()))}


# r[impl kiln.onixos_pilot.closure.preparation]
def admit_preparation_closure(plan, members):
    consumer = set(plan["consumer_derivations"].values()) | set(plan["consumer_outputs"].values())
    overlap = consumer.intersection(members)
    if overlap:
        raise ValueError("preparation reaches consumer construction: " + ", ".join(sorted(overlap)))
    return sorted(set(members))


# r[impl kiln.onixos_pilot.closure.construction]
def consumer_output_status(path, path_info, filesystem_present):
    if set(path_info) != {path}:
        raise ValueError("consumer output observation does not identify the exact path")
    registered = path_info[path] is not None
    return {"registered": registered, "filesystem_present": filesystem_present,
            "present": filesystem_present or registered}
