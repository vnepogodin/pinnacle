-- SPDX-License-Identifier: GPL-3.0-or-later

---@diagnostic disable:redefined-local

---Output management.
---
---An output is what you would call a monitor. It presents windows, your cursor, and other UI elements.
---
---Outputs are uniquely identified by their name, a.k.a. the name of the connector they're plugged in to.
---@class Output
local output = {}

---An output handle.
---
---This is a handle to one of your monitors.
---It serves to make it easier to deal with them, defining methods for getting properties and
---helpers for things like positioning multiple monitors.
---
---This can be retrieved through the various `get` functions in the `Output` module.
---@classmod
---@class OutputHandle A handle to a display.
---@field private _name OutputName The name of this output (or rather, of its connector).
local output_handle = {}

---@param params OutputHandle|string
---@return OutputHandle
local function create_output_from_params(params)
    if type(params) == "table" then
        return params
    end

    return output.get_by_name(params --[[@as string]])
end

---Create a new output handle from a name.
---The name is the unique identifier for each output.
---@param name string
---@return OutputHandle
local function create_output(name)
    ---@type OutputHandle
    local o = { _name = name }
    -- Copy functions over
    for k, v in pairs(output_handle) do
        o[k] = v
    end

    return o
end

---Get this output's name. This is something like "eDP-1" or "HDMI-A-0".
---@return string
function output_handle:name()
    return self._name
end

---Get all tags on this output.
---@return TagHandle[]
---@see Output.tags — The corresponding module function
function output_handle:tags()
    return output.tags(self)
end

---Add tags to this output.
---@param ... string The names of the tags you want to add. You can also pass in a table.
---@overload fun(self: self, tag_names: string[])
---@see Output.add_tags — The corresponding module function
function output_handle:add_tags(...)
    output.add_tags(self, ...)
end

---Get this output's make.
---@return string|nil
---@see Output.make — The corresponding module function
function output_handle:make()
    return output.make(self)
end

---Get this output's model.
---@return string|nil
---@see Output.model — The corresponding module function
function output_handle:model()
    return output.model(self)
end

---Get this output's location in the global space, in pixels.
---@return { x: integer, y: integer }|nil
---@see Output.loc — The corresponding module function
function output_handle:loc()
    return output.loc(self)
end

---Get this output's resolution in pixels.
---@return { w: integer, h: integer }|nil
---@see Output.res — The corresponding module function
function output_handle:res()
    return output.res(self)
end

---Get this output's refresh rate in millihertz.
---For example, 60Hz will be returned as 60000.
---@return integer|nil
---@see Output.refresh_rate — The corresponding module function
function output_handle:refresh_rate()
    return output.refresh_rate(self)
end

---Get this output's physical size in millimeters.
---@return { w: integer, h: integer }|nil
---@see Output.physical_size — The corresponding module function
function output_handle:physical_size()
    return output.physical_size(self)
end

---Get whether or not this output is focused. This is currently defined as having the cursor on it.
---@return boolean|nil
---@see Output.focused — The corresponding module function
function output_handle:focused()
    return output.focused(self)
end

---Set this output's location.
---
---### Examples
---```lua
--- -- Assuming DP-1 is 2560x1440 and DP-2 is 1920x1080...
---local dp1 = output.get_by_name("DP-1")
---local dp2 = output.get_by_name("DP-2")
---
--- -- Place DP-2 to the left of DP-1, top borders aligned
---dp1:set_loc({ x = 1920, y = 0 })
---dp2:set_loc({ x = 0, y = 0 })
---
--- -- Do the same as above, with a different origin
---dp1:set_loc({ x = 0, y = 0 })
---dp2:set_loc({ x = -1920, y = 0 })
---
--- -- Place DP-2 to the right of DP-1, bottom borders aligned
---dp1:set_loc({ x = 0, y = 0 })
---dp2:set_loc({ x = 2560, y = 1440 - 1080 })
---```
---@param loc { x: integer?, y: integer? }
function output_handle:set_loc(loc)
    output.set_loc(self, loc)
end

-- TODO: move this into own file or something ---------------------------------------------

---@alias AlignmentVertical
---| "top" Align the tops of the outputs
---| "center" Center the outputs vertically
---| "bottom" Align the bottoms of the outputs

---@alias AlignmentHorizontal
---| "left" Align the left edges of the outputs
---| "center" Center the outputs vertically
---| "right" Align the right edges of the outputs

---@param op1 OutputHandle
---@param op2 OutputHandle
---@param left_or_right "left" | "right"
---@param alignment AlignmentVertical? How you want to align the `self` output. Defaults to `top`.
local function set_loc_horizontal(op1, op2, left_or_right, alignment)
    local alignment = alignment or "top"
    local self_loc = op1:loc()
    local self_res = op1:res()
    local other_loc = op2:loc()
    local other_res = op2:res()

    if self_loc == nil or self_res == nil or other_loc == nil or other_res == nil then
        return
    end

    ---@type integer
    local x
    if left_or_right == "left" then
        x = other_loc.x - self_res.w
    else
        x = other_loc.x + other_res.w
    end

    if alignment == "top" then
        output.set_loc(op1, { x = x, y = other_loc.y })
    elseif alignment == "center" then
        output.set_loc(op1, { x = x, y = other_loc.y + (other_res.h - self_res.h) // 2 })
    elseif alignment == "bottom" then
        output.set_loc(op1, { x = x, y = other_loc.y + (other_res.h - self_res.h) })
    end
end

---Set this output's location to the right of the specified output.
---
---```
---            top              center            bottom
--- ┌────────┬──────┐ ┌────────┐        ┌────────┐
--- │op      │self  │ │op      ├──────┐ │op      │
--- │        ├──────┘ │        │self  │ │        ├──────┐
--- │        │        │        ├──────┘ │        │self  │
--- └────────┘        └────────┘        └────────┴──────┘
---```
---This will fail if `op` is an invalid output.
---@param op OutputHandle
---@param alignment AlignmentVertical? How you want to align the `self` output. Defaults to `top`.
---@see OutputHandle.set_loc if you need more granular control
function output_handle:set_loc_right_of(op, alignment)
    set_loc_horizontal(self, op, "right", alignment)
end

---Set this output's location to the left of the specified output.
---
---```
---   top              center            bottom
--- ┌──────┬────────┐        ┌────────┐        ┌────────┐
--- │self  │op      │ ┌──────┤op      │        │op      │
--- └──────┤        │ │self  │        │ ┌──────┤        │
---        │        │ └──────┤        │ │self  │        │
---        └────────┘        └────────┘ └──────┴────────┘
---```
---This will fail if `op` is an invalid output.
---@param op OutputHandle
---@param alignment AlignmentVertical? How you want to align the `self` output. Defaults to `top`.
---@see OutputHandle.set_loc if you need more granular control
function output_handle:set_loc_left_of(op, alignment)
    set_loc_horizontal(self, op, "left", alignment)
end

---@param op1 OutputHandle
---@param op2 OutputHandle
---@param top_or_bottom "top" | "bottom"
---@param alignment AlignmentHorizontal? How you want to align the `self` output. Defaults to `top`.
local function set_loc_vertical(op1, op2, top_or_bottom, alignment)
    local alignment = alignment or "left"
    local self_loc = op1:loc()
    local self_res = op1:res()
    local other_loc = op2:loc()
    local other_res = op2:res()

    if self_loc == nil or self_res == nil or other_loc == nil or other_res == nil then
        return
    end

    ---@type integer
    local y
    if top_or_bottom == "top" then
        y = other_loc.y - self_res.h
    else
        y = other_loc.y + other_res.h
    end

    if alignment == "left" then
        output.set_loc(op1, { x = other_loc.x, y = y })
    elseif alignment == "center" then
        output.set_loc(op1, { x = other_loc.x + (other_res.w - self_res.w) // 2, y = y })
    elseif alignment == "right" then
        output.set_loc(op1, { x = other_loc.x + (other_res.w - self_res.w), y = y })
    end
end

---Set this output's location to the top of the specified output.
---
---```
---  left        center      right
--- ┌──────┐    ┌──────┐    ┌──────┐
--- │self  │    │self  │    │self  │
--- ├──────┴─┐ ┌┴──────┴┐ ┌─┴──────┤
--- │op      │ │op      │ │op      │
--- │        │ │        │ │        │
--- └────────┘ └────────┘ └────────┘
---```
---This will fail if `op` is an invalid output.
---@param op OutputHandle
---@param alignment AlignmentHorizontal? How you want to align the `self` output. Defaults to `left`.
---@see OutputHandle.set_loc if you need more granular control
function output_handle:set_loc_top_of(op, alignment)
    set_loc_vertical(self, op, "top", alignment)
end

---Set this output's location to the bottom of the specified output.
---
---```
--- ┌────────┐ ┌────────┐ ┌────────┐
--- │op      │ │op      │ │op      │
--- │        │ │        │ │        │
--- ├──────┬─┘ └┬──────┬┘ └─┬──────┤
--- │self  │    │self  │    │self  │
--- └──────┘    └──────┘    └──────┘
---  left        center      right
---```
---This will fail if `op` is an invalid output.
---@param op OutputHandle
---@param alignment AlignmentHorizontal? How you want to align the `self` output. Defaults to `left`.
---@see OutputHandle.set_loc if you need more granular control
function output_handle:set_loc_bottom_of(op, alignment)
    set_loc_vertical(self, op, "bottom", alignment)
end

------------------------------------------------------

---Get an output by its name.
---
---"Name" in this sense does not mean its model or manufacturer;
---rather, "name" is the name of the connector the output is connected to.
---This should be something like "HDMI-A-0", "eDP-1", or similar.
---
---### Example
---```lua
---local monitor = output.get_by_name("DP-1")
---print(monitor:name()) -- should print `DP-1`
---```
---@param name string The name of the output.
---@return OutputHandle output The output. If it doesn't exist, a dummy handle with the name "" will be returned.
function output.get_by_name(name)
    local response = Request("GetOutputs")
    local output_names = response.RequestResponse.response.Outputs.output_names

    for _, output_name in pairs(output_names) do
        if output_name == name then
            return create_output(output_name)
        end
    end

    return create_output("")
end

---Get outputs by their model.
---
---Note: This may or may not be what is reported by other monitor listing utilities.
---Pinnacle currently fails to pick up one of my monitor's models when it is correctly
---picked up by tools like wlr-randr. I'll fix this in the future.
---
---This is something like "DELL E2416H" or whatever gibberish monitor manufacturers call their displays.
---@param model string The model of the output(s).
---@return OutputHandle[] outputs All outputs with this model.
function output.get_by_model(model)
    local response = Request("GetOutputs")
    local output_names = response.RequestResponse.response.Outputs.output_names

    ---@type OutputHandle[]
    local outputs = {}
    for _, output_name in pairs(output_names) do
        local o = create_output(output_name)
        if o:model() == model then
            table.insert(outputs, o)
        end
    end

    return outputs
end

---Get outputs by their resolution.
---
---@param width integer The width of the outputs, in pixels.
---@param height integer The height of the outputs, in pixels.
---@return OutputHandle[] outputs All outputs with this resolution.
function output.get_by_res(width, height)
    local response = Request("GetOutputs")

    local output_names = response.RequestResponse.response.Outputs.output_names

    ---@type OutputHandle[]
    local outputs = {}
    for _, output_name in pairs(output_names) do
        local o = create_output(output_name)
        if o:res() and o:res().w == width and o:res().h == height then
            table.insert(outputs, o)
        end
    end

    return outputs
end

---Get the currently focused output. This is currently implemented as the one with the cursor on it.
---
---If you have no outputs plugged in, this will return a dummy `OutputHandle` with the name "".
---@return OutputHandle output The output, or a dummy handle if none are focused.
function output.get_focused()
    local response = Request("GetOutputs")
    local output_names = response.RequestResponse.response.Outputs.output_names

    for _, output_name in pairs(output_names) do
        local o = create_output(output_name)
        if o:focused() then
            return o
        end
    end

    return create_output("")
end

---Connect a function to be run on all current and future outputs.
---
---When called, `connect_for_all` will immediately run `func` with all currently connected outputs.
---If a new output is connected, `func` will also be called with it.
---
---This will *not* be called if it has already been called for a given connector.
---This means turning your monitor off and on or unplugging and replugging it *to the same port*
---won't trigger `func`. Plugging it in to a new port *will* trigger `func`.
---This is intended to prevent duplicate setup.
---
---Please note: this function will be run *after* Pinnacle processes your entire config.
---For example, if you define tags in `func` but toggle them directly after `connect_for_all`,
---nothing will happen as the tags haven't been added yet.
---@param func fun(output: OutputHandle) The function that will be run.
function output.connect_for_all(func)
    ---@param args Args
    table.insert(CallbackTable, function(args)
        local args = args.ConnectForAllOutputs
        func(create_output(args.output_name))
    end)
    SendMsg({
        ConnectForAllOutputs = {
            callback_id = #CallbackTable,
        },
    })
end

---Get the output the specified tag is on.
---@param tag TagHandle
---@return OutputHandle
---@see Tag.output — A global method for fully qualified syntax (for you Rustaceans out there)
---@see TagHandle.output — The corresponding object method
function output.get_for_tag(tag)
    local response = Request({
        GetTagProps = {
            tag_id = tag:id(),
        },
    })
    local output_name = response.RequestResponse.response.TagProps.output_name

    return create_output(output_name or "")
end

---------Fully-qualified functions

---Get the specified output's make.
---@param op OutputHandle|string The name of the output or an output handle.
---@return string|nil
---@see OutputHandle.make — The corresponding object method
function output.make(op)
    local op = create_output_from_params(op)

    local response = Request({
        GetOutputProps = {
            output_name = op:name(),
        },
    })
    local props = response.RequestResponse.response.OutputProps
    return props.make
end

---Get the specified output's model.
---@param op OutputHandle|string The name of the output or an output object.
---@return string|nil
---@see OutputHandle.model — The corresponding object method
function output.model(op)
    local op = create_output_from_params(op)

    local response = Request({
        GetOutputProps = {
            output_name = op:name(),
        },
    })
    local props = response.RequestResponse.response.OutputProps
    return props.model
end

---Get the specified output's location in the global space, in pixels.
---@param op OutputHandle|string The name of the output or an output object.
---@return { x: integer, y: integer }|nil
---@see OutputHandle.loc — The corresponding object method
function output.loc(op)
    local op = create_output_from_params(op)

    local response = Request({
        GetOutputProps = {
            output_name = op:name(),
        },
    })
    local props = response.RequestResponse.response.OutputProps
    if props.loc == nil then
        return nil
    else
        return { x = props.loc[1], y = props.loc[2] }
    end
end

---Get the specified output's resolution in pixels.
---@param op OutputHandle|string The name of the output or an output object.
---@return { w: integer, h: integer }|nil
---@see OutputHandle.res — The corresponding object method
function output.res(op)
    local op = create_output_from_params(op)

    local response = Request({
        GetOutputProps = {
            output_name = op:name(),
        },
    })
    local props = response.RequestResponse.response.OutputProps
    if props.res == nil then
        return nil
    else
        return { w = props.res[1], h = props.res[2] }
    end
end

---Get the specified output's refresh rate in millihertz.
---For example, 60Hz will be returned as 60000.
---@param op OutputHandle|string The name of the output or an output object.
---@return integer|nil
---@see OutputHandle.refresh_rate — The corresponding object method
function output.refresh_rate(op)
    local op = create_output_from_params(op)

    local response = Request({
        GetOutputProps = {
            output_name = op:name(),
        },
    })
    local props = response.RequestResponse.response.OutputProps
    return props.refresh_rate
end

---Get the specified output's physical size in millimeters.
---@param op OutputHandle|string The name of the output or an output object.
---@return { w: integer, h: integer }|nil
---@see OutputHandle.physical_size — The corresponding object method
function output.physical_size(op)
    local op = create_output_from_params(op)

    local response = Request({
        GetOutputProps = {
            output_name = op:name(),
        },
    })
    local props = response.RequestResponse.response.OutputProps
    if props.physical_size == nil then
        return nil
    else
        return { w = props.physical_size[1], h = props.physical_size[2] }
    end
end

---Get whether or not the specified output is focused. This is currently defined as having the cursor on it.
---@param op OutputHandle|string The name of the output or an output object.
---@return boolean|nil
---@see OutputHandle.focused — The corresponding object method
function output.focused(op)
    local op = create_output_from_params(op)

    local response = Request({
        GetOutputProps = {
            output_name = op:name(),
        },
    })
    local props = response.RequestResponse.response.OutputProps
    return props.focused
end

---Get the specified output's tags.
---@param op OutputHandle|string The name of the output or an output object.
---@return TagHandle[]
---@see Tag.get_on_output — The called function
---@see OutputHandle.tags — The corresponding object method
function output.tags(op)
    local op = create_output_from_params(op)

    return require("tag").get_on_output(op)
end

---Add tags to the specified output.
---@param op OutputHandle|string The name of the output or an output object.
---@param ... string The names of the tags you want to add. You can also pass in a table.
---@overload fun(op: OutputHandle|string, tag_names: string[])
---@see Tag.add — The called function
---@see OutputHandle.add_tags — The corresponding object method
function output.add_tags(op, ...)
    local op = create_output_from_params(op)

    require("tag").add(op, ...)
end

---Set the specified output's location.
---
---### Examples
---```lua
--- -- Assuming DP-1 is 2560x1440 and DP-2 is 1920x1080...
---local dp1 = output.get_by_name("DP-1")
---local dp2 = output.get_by_name("DP-2")
---
--- -- Place DP-2 to the left of DP-1, top borders aligned
---output.set_loc(dp1, { x = 1920, y = 0 })
---output.set_loc(dp2, { x = 0, y = 0 })
---
--- -- Do the same as above, with a different origin
---output.set_loc(dp1, { x = 0, y = 0 })
---output.set_loc(dp2, { x = -1920, y = 0 })
---
--- -- Place DP-2 to the right of DP-1, bottom borders aligned
---output.set_loc(dp1, { x = 0, y = 0 })
---output.set_loc(dp2, { x = 2560, y = 1440 - 1080 })
---```
---@param op OutputHandle|string The name of the output or an output object.
---@param loc { x: integer?, y: integer? }
function output.set_loc(op, loc)
    local op = create_output_from_params(op)

    SendMsg({
        SetOutputLocation = {
            output_name = op:name(),
            x = loc.x,
            y = loc.y,
        },
    })
end

return output
