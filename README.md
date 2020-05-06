# TRACER

```sh
$ cargo run --example synchronous
...
...
...
======================================================================   2.95 ms root
==                                                                       0.09 ms func1
=                                                                        0.08 ms func2 😻
  ====                                                                   0.17 ms func1
   ==                                                                    0.10 ms func2 😻
      =====                                                              0.22 ms func1
        ==                                                               0.12 ms func2 😻
           ======                                                        0.27 ms func1
               ==                                                        0.11 ms func2 😻
                  ======                                                 0.27 ms func1
                      ==                                                 0.09 ms func2 😻
                        ======                                           0.29 ms func1
                             =                                           0.08 ms func2 😻
                               =======                                   0.32 ms func1
                                     =                                   0.07 ms func2 😻
                                       =========                         0.41 ms func1
                                               =                         0.07 ms func2 😻
                                                 =========               0.41 ms func1
                                                         =               0.07 ms func2 😻
                                                           ==========    0.45 ms func1
                                                                   =     0.08 ms func2 😻
```

```sh
$ cargo run --example asynchronous
...
...
...
==============================================                           1.10 ms root
     ===                                                                 0.09 ms parallel_job
      ========================                                           0.57 ms parallel_iter_job
                     =======                                             0.18 ms other_job 💯
      ================================================                   1.13 ms parallel_iter_job
                                      ===============                    0.37 ms other_job 💯
        =====================================================            1.25 ms parallel_iter_job
                                                         ===             0.08 ms other_job 💯
       ==============================================================    1.47 ms parallel_iter_job
                                                               =====     0.13 ms other_job 💯
           ==================================                            0.81 ms other_job 💯
```
