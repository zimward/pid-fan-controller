# About

A fan controller that employs a PID instead of a usual P control-loop. This is supposed to improve response times,
reduce ocillations and steady-state error.

# Configuration

The configuration file is hardcoded to /etc/pid-fan-settings.json.

```
{
  //interval of the controller in ms
  //default 500
  "interval":500,

  //heat pressure sources. (devices to be cooled)
  "heat_srcs":[
    {
      //unique identifier of the heat source
      "name":"cpu",
      
      //path to the hwmon temp_input file of the device
      //can contain wildcards but must only resolve to one result.
      "wildcard_path":"/sys/devices/platform/nct6775.*/hwmon/hwmon*/temp1_input",
      
      //parameters of the PID controller
      "PID_params":{
      
        //setpoint of the controller in °C
        "set_point":60,
        
        //Kp
        "P":,
        
        //Ki
        "I":,
      
        //Kd
        "D":,
      }
    }
  ],
  //fans to be controlled
  "fans":[
    {
      //name of the fan, can be ommited
      "name":"cpu fan",
      
      //path to the hwmon pwm file of the fan.
      //can contain wildcards but must only resolve to one result.
      "wildcard_path":"/sys/devices/platform/nct6775.*/hwmon/hwmon*/pwm1",
      
      //minimum pwm value
      "min_pwm":10,
      
      //maximum pwm value
      "max_pwm":255,
      
      //whether to set pwm to 0 when min_pwm is reached.
      //intended for fans which stop spinning before pwm reaches 0
      "cutoff":true,
      
      //heat pressure sources which controll the fan.
      //must only include names of heat pressure sources defined above.
      "heat_pressure_srcs":["cpu"]
    }
  ]
}
```

# NixOs Flake
Import pid-fan-control:
```
{
  inputs={
    pid-fan-control.url = "github:zimward/PID-fan-control";
  };
  outputs={self,nixpkgs,pid-fan-control}:{
    nixosConfigurations.mySystem =nixpkgs.lib.nixosSystem {
      modules=[
        inputs.pid-fan-control.nixosModules.default
        ./whatever/modules/your/config/has.nix
      ];
    };
  };
}
```
Configuration of the controler service
```
  pid-fan-controller = {
    enable = true;
    settings={
      #attrset containing the same entries as the config shown above
    }
  };

```
