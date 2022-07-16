# Motivation

In the last decade the hardware has generally improved and gotten more efficient,
but fan control is still a mess. The "smart" fan controllers on any motherboard
i encountered just use linear logic or lookup tables with the granularity of boulders.  

A [PID controller](https://en.wikipedia.org/wiki/PID_controller) is genrally
much more suitable for this job since it adjusts to the error until it reaches
it's setpoint (if it's possible), doesn't over react to temperature spikes
if properly tuned and adjusts for external variables (ambient temperature) to a
certain degree.

## Why i wrote this

The controller from ThunderMikey is basically the same from the way it works,
but i personally dislike YAML as a format and wanted the option to have fans
cut-out completely if the load allows for it. (What i mean by this is to set the
fan's pwm value to 0 if a threshold is reached since my fans stop spinning
before reaching 0 and get hot if kept powered without spinning)  

Additionally i wanted to reduce the resource consumption of the controller (which
was already low) since python has some overhead. But since this program has to
run with root privileges i didn't want to introduce weakpoints by using C which
gets unsafe quite easily. (if you find an issue please contact me, open a issue
or create a PR)

# Similar works

- [pid_fan_controller](https://github.com/ThunderMikey/pid_fan_controller)
- [macbookfan](https://github.com/jbg/macbookfan)

# Configuration

The configuration file is hardcoded to be /etc/pid-fan-settings.json ,
all keys are present in the [example_fan_settings.json](https://github.com/zimward/PID-fan-control/blob/master/example_fan_settings.json) with the right
structure. "wildcard_path" is the path pointing either to the hwmon tempX_input
for heat sources or the pwmX file of the fan, can contain multiple wildcards but
has to resolve to only one result, there is currently no way to configure
temperature input from nvidia card via their nvidia-smi cli tool (i may implement
this at some point). The "name" key is used to assing heat sources
to fans (the "name" key is just a comment for fans since json has no comments).
The "heat_pressure_srcs" array assigns heat sources to fans using their name, a
fan must have at least one heat source assigned. Keys which got a default value
assigned can be omitted and are listed in the table below.

| Key      |Unit | Default value | Description                                       |
|----------|-----|---------------|---------------------------------------------------|
| interval | ms  | 500ms         | interval between pid runs                         |
| min_pwm  | -   | 0             | min. pwm value (set if unsure)                    |
| max_pwm  | -   | 255           | max. pwm value                                    |
| cutoff   | bool| false         | if min_pwm is reached the fan is set to 0         |

# Performance

**I didn't try to control the environment too hard, so take those numbers
with a grain of salt. All tests did run on a i5-6600k@~1GHz with the
same configuration at an 500ms interval.**  

|      |Rust      | Python   |
|------|----------|----------|
|Memory| 1204kb   | 11176kb  |
|Real  | 5:06 min | 5:00 min |
|User  | 0.06s    | 0.31s    |
|System| 0.15s    | 0.20s    |

The largest difference is the memory consumption which seems to be at least 10x
lower on the rust implementation. The CPU time consumption of the rust
implementation is about 5x lower in userspace (the actual PID calculation) and
about the same in system/kernel space which is caused by the sysfs io (my guess
to why it's so slow is that the chip which the values get written to are pretty
slow and the kernel waits for the write to finish in case it fails).  

In my real-world usage the rust controller uses about 1s of CPU time per hour.
