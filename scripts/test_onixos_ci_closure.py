import copy
import unittest

from onixos_ci_closure import admit_preparation_closure, consumer_output_status, preparation_plan


def consumer_jobs():
    return [
        {"attr": "home-activation", "drvPath": "launcher.drv", "outputs": {"out": "launcher"},
         "inputDrvs": {"activation.drv": ["out"], "shell.drv": ["out"]}},
        {"attr": "activation", "drvPath": "activation.drv", "outputs": {"out": "activation"},
         "inputDrvs": {"files.drv": ["out"], "shell.drv": ["out"]}},
        {"attr": "home-files", "drvPath": "files.drv", "outputs": {"out": "files"},
         "inputDrvs": {"toolchain.drv": ["out"], "shell.drv": ["out"]}},
    ]


# r[verify kiln.onixos_pilot.closure.preparation]
class PreparationAdmission(unittest.TestCase):
    def test_shared_external_inputs_are_prepared_without_consumer_builds(self):
        plan = preparation_plan(list(reversed(consumer_jobs())))
        self.assertEqual(plan["dependency_derivations"], ["shell.drv", "toolchain.drv"])
        self.assertEqual(admit_preparation_closure(plan, ["shell.drv", "source", "toolchain.drv", "source"]),
                         ["shell.drv", "source", "toolchain.drv"])

    def test_indirect_consumer_derivation_or_output_cannot_be_prepared(self):
        plan = preparation_plan(consumer_jobs())
        for member in ["launcher.drv", "activation.drv", "files.drv", "launcher", "activation", "files"]:
            with self.subTest(member=member), self.assertRaisesRegex(ValueError, "reaches consumer"):
                admit_preparation_closure(plan, ["toolchain.drv", member])

    def test_missing_duplicate_or_errored_artifacts_fail_closed(self):
        jobs = consumer_jobs()
        errored = copy.deepcopy(jobs)
        errored[1]["error"] = "evaluation failed"
        for invalid in [jobs[:-1], jobs + [jobs[0]], [jobs[0], jobs[1], jobs[1]], errored]:
            with self.subTest(jobs=invalid), self.assertRaises(ValueError):
                preparation_plan(invalid)

    def test_disconnected_generated_output_is_not_a_launcher_closure(self):
        for index, dependency in [(0, "activation.drv"), (1, "files.drv")]:
            jobs = consumer_jobs()
            del jobs[index]["inputDrvs"][dependency]
            with self.subTest(index=index), self.assertRaisesRegex(ValueError, "real launcher closure"):
                preparation_plan(jobs)

    def test_aliased_or_multi_output_artifacts_are_rejected(self):
        for kind in ["derivation", "output", "extra-output"]:
            jobs = consumer_jobs()
            if kind == "derivation":
                jobs[2]["drvPath"] = jobs[1]["drvPath"]
            elif kind == "output":
                jobs[2]["outputs"]["out"] = jobs[1]["outputs"]["out"]
            else:
                jobs[2]["outputs"]["debug"] = "debug"
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                preparation_plan(jobs)


# r[verify kiln.onixos_pilot.closure.construction]
class ConsumerOutputObservation(unittest.TestCase):
    def test_native_null_is_absent_even_when_the_command_succeeds(self):
        self.assertEqual(consumer_output_status("files", {"files": None}, False),
                         {"registered": False, "filesystem_present": False, "present": False})

    def test_registered_output_blocks_fresh_construction(self):
        status = consumer_output_status("files", {"files": {"narHash": "native"}}, True)
        self.assertTrue(status["registered"])
        self.assertTrue(status["present"])

    def test_failed_unregistered_partial_directory_is_not_a_realised_output(self):
        self.assertEqual(consumer_output_status("files", {"files": None}, True),
                         {"registered": False, "filesystem_present": True, "present": True})

    def test_missing_or_substituted_native_identity_is_rejected(self):
        for path_info in [{}, {"other": None}, {"files": None, "other": None}]:
            with self.subTest(path_info=path_info), self.assertRaises(ValueError):
                consumer_output_status("files", path_info, False)


if __name__ == "__main__":
    unittest.main()
