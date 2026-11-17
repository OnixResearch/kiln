(import "../web-campaign-controls.ncl") ((import "web-campaign-controls-inputs.ncl") & { grants | force = [{ principal = "local:operator", campaigns = ["selected-cairn"], commands = ["deploy"] }] })
