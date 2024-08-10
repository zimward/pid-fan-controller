self:
{
  lib,
  config,
  pkgs,
  ...
}:
let
  cfg = config.pid-fan-controller;
  fOpt = (
    descr:
    lib.mkOption {
      type = lib.types.float;
      description = descr;
    }
  );
  heat_src =
    { ... }:
    {
      options = {
        name = lib.mkOption {
          type = lib.types.uniq lib.types.nonEmptyStr;
          description = "name of heat source";
        };
        wildcard_path = lib.mkOption {
          type = lib.types.nonEmptyStr;
          description = "wildcard path of heat source temp_input, can contain wildcards";
        };
        PID_params = {
          set_point = lib.mkOption {
            type = lib.types.ints.unsigned;
            description = "set point of controller";
          };
          P = fOpt "K_p of PID controller";
          I = fOpt "K_i of PID controller";
          D = fOpt "K_d of PID controller";
        };
      };
    };

  fan =
    { ... }:
    {
      options = {
        name = lib.mkOption {
          default = "";
          type = lib.types.str;
          description = "name to identify fan";
        };
        wildcard_path = lib.mkOption {
          type = lib.types.str;
          description = "wildcard path of fan pwm file";
        };
        min_pwm = lib.mkOption {
          default = 0;
          type = lib.types.ints.between 0 255;
          description = "minimum PWM fan speed";
        };
        max_pwm = lib.mkOption {
          default = 255;
          type = lib.types.ints.between 0 255;
          description = "maximum PWM fan speed";
        };
        cutoff = lib.mkOption {
          default = false;
          type = lib.types.bool;
          description = "whether to stop fan when min_pwm is reached (eg if fan stopps spinning when min_pwm is reached)";
        };
        heat_pressure_srcs = lib.mkOption {
          type = lib.types.nonEmptyListOf (lib.types.enum (map (heat: heat.name) cfg.settings.heat_srcs));
          description = "heat pressure sources which are affected by the fan";
        };
      };
    };
in
{
  options = {
    pid-fan-controller = {
      enable = lib.mkOption {
        default = false;
        description = "Enable PID fan controller";
      };
      package = lib.mkOption {
        description = "PID fan controller package to use";
        type = lib.types.package;
        default = self.packages.${pkgs.system}.default;
      };
      settings = {
        interval = lib.mkOption {
          default = 500;
          type = lib.types.int;
          description = "Interval between controller cycels.";
        };
        heat_srcs = lib.mkOption {
          type = lib.types.listOf (lib.types.submodule heat_src);
          description = "list of heat sources";
        };
        fans = lib.mkOption {
          type = lib.types.listOf (lib.types.submodule fan);
          description = "list of fans";
        };
      };
    };
  };
  config = lib.mkIf cfg.enable {
    environment.etc."pid-fan-settings.json".text = builtins.toJSON cfg.settings;
    #load nct6775 module on boot to expose additional hardware
    boot.kernelModules = [ "nct6775" ];
    systemd.services.pid-fan-controller = {
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "simple";
        ExecStart = [ "${cfg.package}/bin/pid-fan-controller" ];
        ExecStopPost = [ "${cfg.package}/bin/pid-fan-controller disable" ];
        Restart = "always";
        #This service needs to run as root to write to /sys. 
        #therefore it should operate with the least amount of priviledges needed
        ProtectHome = "yes";
        #strict is not possible as it needs /sys
        ProtectSystem = "full";
        ProtectProc = "invisible";
        PrivateNetwork = "yes";
        NoNewPrivileges = "yes";
        MemoryDenyWriteExecute = "yes";
        RestrictNamespaces = "~user pid net uts mnt";
        ProtectKernelModules = "yes";
        RestrictRealtime = "yes";
        SystemCallFilter = "@system-service";
        CapabilityBoundingSet = "~CAP_KILL CAP_WAKE_ALARM CAP_IPC_LOC CAP_BPF CAP_LINUX_IMMUTABLE CAP_BLOCK_SUSPEND CAP_MKNOD";
      };
      # restart unit if config changed
      restartTriggers = [ config.environment.etc."pid-fan-settings.json".text ];
    };
    systemd.services.pid-fan-controller-sleep = {
      before = [ "sleep.target" ];
      wantedBy = [ "sleep.target" ];
      unitConfig = {
        StopWhenUnneeded = "yes";
      };
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = [ "${pkgs.systemd}/bin/systemctl stop pid-fan-controller.service" ];
        ExecStop = [ "${pkgs.systemd}/bin/systemctl restart pid-fan-controller.service" ];
      };
    };
  };
}
