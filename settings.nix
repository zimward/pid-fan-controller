self:
{
  lib,
  config,
  pkgs,
  ...
}:
let
  cfg = config.pid-fan-controller;
  fOpt =
    with lib;
    (
      descr:
      mkOption {
        type = types.float;
        description = descr;
      }
    );
  heat_src =
    { ... }:
    {
      options = with lib; {
        name = mkOption {
          type = with types; uniq nonEmptyStr;
          description = "name of heat source";
        };
        wildcard_path = mkOption {
          type = types.nonEmptyStr;
          description = "wildcard path of heat source temp_input, can contain wildcards";
        };
        PID_params = {
          set_point = mkOption {
            type = with types; ints.unsigned;
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
      options = with lib; {
        name = mkOption {
          default = "";
          type = types.str;
          description = "name to identify fan";
        };
        wildcard_path = mkOption {
          type = types.str;
          description = "wildcard path of fan pwm file";
        };
        min_pwm = mkOption {
          default = 0;
          type = types.ints.between 0 255;
          description = "minimum PWM fan speed";
        };
        max_pwm = mkOption {
          default = 255;
          type = types.ints.between 0 255;
          description = "maximum PWM fan speed";
        };
        cutoff = mkOption {
          default = false;
          type = types.bool;
          description = "whether to stop fan when min_pwm is reached (eg if fan stopps spinning when min_pwm is reached)";
        };
        heat_pressure_srcs = mkOption {
          type = with types; nonEmptyListOf (enum (map (heat: heat.name) cfg.settings.heat_srcs));
          description = "heat pressure sources which are affected by the fan";
        };
      };
    };
in
{
  options = with lib; {
    pid-fan-controller = {
      enable = mkOption {
        default = false;
        description = "Enable PID fan controller";
      };
      package = mkOption {
        description = "PID fan controller package to use";
        type = types.package;
        default = self.packages.${pkgs.system}.default;
      };
      settings = {
        interval = mkOption {
          default = 500;
          type = types.int;
          description = "Interval between controller cycels.";
        };
        heat_srcs = mkOption {
          type = with types; listOf (submodule heat_src);
          description = "list of heat sources";
        };
        fans = mkOption {
          type = with types; listOf (submodule fan);
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
        CapabilityBoundingSet = "~CAP_*";
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
