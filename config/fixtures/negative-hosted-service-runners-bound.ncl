let positive = import "hosted-service-positive.ncl" in
let base = positive.execution in
let local = base.runners.local in
(import "../hosted-service.ncl") (
  positive
  & {
    execution | force =
      base
      & {
        runners | force = {
          r0 = local,
          r1 = local,
          r2 = local,
          r3 = local,
          r4 = local,
          r5 = local,
          r6 = local,
          r7 = local,
          r8 = local,
          r9 = local,
          r10 = local,
          r11 = local,
          r12 = local,
          r13 = local,
          r14 = local,
          r15 = local,
          r16 = local,
          r17 = local,
          r18 = local,
          r19 = local,
          r20 = local,
          r21 = local,
          r22 = local,
          r23 = local,
          r24 = local,
          r25 = local,
          r26 = local,
          r27 = local,
          r28 = local,
          r29 = local,
          r30 = local,
          r31 = local,
          r32 = local,
        }
      }
  }
)
