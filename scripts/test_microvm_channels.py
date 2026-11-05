"""Consumer-visible channel decoding and placement qualification boundaries."""
import importlib.util
from pathlib import Path
import os
import subprocess
import sys
import unittest

SPEC = importlib.util.spec_from_file_location("channels", Path(__file__).with_name("microvm-channels.py"))
CHANNELS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHANNELS)


def endpoint(cpu, core, llc, pool, host="machine-a"):
    return {"os_instance_label":host,"cpus":[cpu],"endpoint":{"pool":pool},
            "topology":{str(cpu):{"physical_package_id":"0","core_id":str(core),"llc_shared_cpus":llc}}}


class ChannelQualification(unittest.TestCase):
    def test_held_out_decoding_uses_only_training_threshold(self):
        bits = CHANNELS.TRAINING + "01101001"
        samples = [100 if bit=="0" else 20 for bit in bits]
        samples[-1] = 100  # One real receiver error, not a fitted held-out threshold.
        result = CHANNELS.reduce_samples(bits,samples,50)
        self.assertEqual(result["bit_errors"],1)
        self.assertEqual(result["bit_error_rate"],1/8)
        self.assertEqual(result["raw_bitrate_bps"],20)
        with self.assertRaises(ValueError):
            CHANNELS.reduce_samples(bits,samples[:-1],50)
        with self.assertRaises(ValueError):
            CHANNELS.reduce_samples("1"+bits[1:],samples,50)

    def test_pool_names_do_not_prove_llc_separation(self):
        first=endpoint(0,0,"0-7,16-23","pool-a")
        same_llc=endpoint(1,1,"0-7,16-23","pool-b")
        with self.assertRaisesRegex(ValueError,"share an LLC"):
            CHANNELS.validate_placement("separate_pools",first,same_llc)
        other_llc=endpoint(8,8,"8-15,24-31","pool-b")
        self.assertEqual(CHANNELS.validate_placement("separate_pools",first,other_llc),"distinct_llc_same_observed_os_instance_measurement_only")
        other_llc["endpoint"]["pool"]="pool-a"
        with self.assertRaisesRegex(ValueError,"pool identities"):
            CHANNELS.validate_placement("separate_pools",first,other_llc)

    def test_pinned_smt_siblings_are_not_separate_cores(self):
        first=endpoint(0,0,"0-7,16-23","a")
        sibling=endpoint(16,0,"0-7,16-23","b")
        with self.assertRaisesRegex(ValueError,"physical core"):
            CHANNELS.validate_placement("pinned_cores",first,sibling)
        self.assertEqual(CHANNELS.validate_placement("pinned_cores",first,endpoint(1,1,"0-7,16-23","b")),"same_observed_os_instance_physical_placement_unverified")
        # Distinct containers may share one physical machine and the same LLC.
        remote=endpoint(0,0,"0-7,16-23","b",host="different-container-instance")
        self.assertEqual(CHANNELS.validate_placement("separate_pools",first,remote),"distinct_observed_os_instances_physical_placement_unverified")

    def test_full_admitted_schedule_is_not_cut_off_at_180_seconds(self):
        request={"role":"receiver","count":1056,"symbol_ms":1000,"start_ms":40000}
        self.assertEqual(CHANNELS.schedule_timeout(request,now_ms=10000),1096)
        request["count"]=1057
        with self.assertRaises(ValueError):
            CHANNELS.schedule_timeout(request,now_ms=10000)
        request.update(count=1056,symbol_ms=1001)
        with self.assertRaises(ValueError):
            CHANNELS.schedule_timeout(request,now_ms=10000)

    def test_owned_process_is_reaped_on_timeout_and_cancellation(self):
        for failure in (subprocess.TimeoutExpired("probe",0), KeyboardInterrupt()):
            with self.assertRaises(type(failure)):
                with CHANNELS.owned_process([sys.executable,"-c","import time; print('ready',flush=True); time.sleep(30)"],stdout=subprocess.PIPE,text=True) as process:
                    self.assertEqual(process.stdout.readline().strip(),"ready")
                    pid=process.pid
                    raise failure
            self.assertIsNotNone(process.poll())
            with self.assertRaises(ProcessLookupError):
                os.kill(pid,0)

    def test_os_identity_is_private_report_scoped_not_physical_attestation(self):
        first=CHANNELS.os_instance_label("01"*32,"confidential-id")
        self.assertNotIn("confidential-id",first)
        self.assertNotEqual(first,CHANNELS.os_instance_label("02"*32,"confidential-id"))
        self.assertEqual(first,CHANNELS.os_instance_label("01"*32,"confidential-id"))


if __name__=="__main__":
    unittest.main()
