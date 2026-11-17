(import "../web-campaign-controls.ncl") ((import "web-campaign-controls-inputs.ncl") & { grants | force = [{ principal = "*", campaigns = ["selected-cairn"], commands = ["pause"] }] })
