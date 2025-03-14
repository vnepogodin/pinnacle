-- SPDX-License-Identifier: GPL-3.0-or-later

---Tag management.
---
---This module provides utilities for creating and manipulating tags.
---
---A tag is a sort of marker for each of your windows. It allows you to present windows in ways that
---traditional workspaces cannot.
---
---More specifically:
---
--- - A window can have multiple tags.
---     - This means that you can have one window show up across multiple "workspaces" if you come
---       something like i3.
--- - An output can display multiple tags at once.
---     - This allows you to toggle a tag and have windows on both tags display at once.
---       This is helpful if you, say, want to reference a browser window while coding; you toggle your
---       browser's tag and temporarily reference it while you work without having to change screens.
---
---Many of the functions in this module take `TagConstructor`.
---This is a convenience so you don't have to get a tag handle every time you want to do
---something with tags.
---
---Instead, you can pass in either:
---
--- - A string of the tag's name (ex. "1")
---     - This will get the first tag with that name on the focused output.
--- - A table where `name` is the name and `output` is the output (or its name) (ex. { name = "1", output = "DP-1" })
---     - This will get the first tag with that name on the specified output.
--- - A tag handle itself
---     - If you already have a tag handle, it will be used directly.
---
---If you need to get tags beyond the first with the same name, use a `get` function and find what you need.
---@class Tag
local tag = {}

---@alias Layout
---| "MasterStack" # One master window on the left with all other windows stacked to the right.
---| "Dwindle" # Windows split in half towards the bottom right corner.
---| "Spiral" # Windows split in half in a spiral.
---| "CornerTopLeft" # One main corner window in the top left with a column of windows on the right and a row on the bottom.
---| "CornerTopRight" # One main corner window in the top right with a column of windows on the left and a row on the bottom.
---| "CornerBottomLeft" # One main corner window in the bottom left with a column of windows on the right and a row on the top.
---| "CornerBottomRight" # One main corner window in the bottom right with a column of windows on the left and a row on the top.

---@alias TagTable { name: string, output: (string|OutputHandle)? }

---@alias TagConstructor TagHandle|TagTable|string

---A tag handle.
---
---This is a handle to a tag that can be passed to windows and such.
---
---This can be retrieved through the various `get` functions in the `Tag` module.
---@classmod
---@class TagHandle
---@field private _id TagId The internal id of this tag.
local tag_handle = {}

---Create a tag from an id.
---The id is the unique identifier for each tag.
---@param id TagId
---@return TagHandle
local function create_tag(id)
    ---@type TagHandle
    local t = { _id = id }
    -- Copy functions over
    for k, v in pairs(tag_handle) do
        t[k] = v
    end

    return t
end

---Get this tag's internal id.
---***You probably won't need to use this.***
---@return TagId
function tag_handle:id()
    return self._id
end

---Get this tag's active status.
---@return boolean|nil active `true` if the tag is active, `false` if not, and `nil` if the tag doesn't exist.
---@see Tag.active — The corresponding module function
function tag_handle:active()
    return tag.active(self)
end

---Get this tag's name.
---@return string|nil name The name of this tag, or nil if it doesn't exist.
---@see Tag.name — The corresponding module function
function tag_handle:name()
    return tag.name(self)
end

---Get this tag's output.
---@return OutputHandle output The output this tag is on, or a dummy handle if the tag doesn't exist.
---@see Tag.output — The corresponding module function
function tag_handle:output()
    return tag.output(self)
end

---Switch to this tag.
---@see Tag.switch_to — The corresponding module function
function tag_handle:switch_to()
    tag.switch_to(self)
end

---Toggle this tag.
---@see Tag.toggle — The corresponding module function
function tag_handle:toggle()
    tag.toggle(self)
end

---Set this tag's layout.
---@param layout Layout
---@see Tag.set_layout — The corresponding module function
function tag_handle:set_layout(layout)
    tag.set_layout(self, layout)
end

-----------------------------------------------------------

---Add tags to the specified output.
---
---### Examples
---```lua
---local op = output.get_by_name("DP-1")
---if op ~= nil then
---    tag.add(op, "1", "2", "3", "4", "5") -- Add tags with names 1-5
---end
---
--- -- You can also pass in a table.
---local tags = {"Terminal", "Browser", "Code", "Potato", "Email"}
---tag.add(op, tags)
---```
---@param output OutputHandle The output you want these tags to be added to.
---@param ... string The names of the new tags you want to add.
---@overload fun(output: OutputHandle, tag_names: string[])
---@see OutputHandle.add_tags — The corresponding object method
function tag.add(output, ...)
    local varargs = { ... }
    if type(varargs[1]) == "string" then
        local tag_names = varargs
        tag_names["n"] = nil -- remove the length to make it a true array for serializing

        SendMsg({
            AddTags = {
                output_name = output:name(),
                tag_names = tag_names,
            },
        })
    else
        local tag_names = varargs[1] --[=[@as string[]]=]

        SendMsg({
            AddTags = {
                output_name = output:name(),
                tag_names = tag_names,
            },
        })
    end
end

---Toggle a tag on the specified output. If the output isn't specified, toggle it on the currently focused output instead.
---
---### Example
---
---```lua
---local op = output.get_by_name("DP-1")
---
---tag.toggle("1")             -- Toggle tag 1 on the focused output
---
---tag.toggle({ name = "1", output = "DP-1" }) -- Toggle tag 1 on "DP-1"
---tag.toggle({ name = "1", output = op })     -- Same as above
---
--- -- Using a tag handle
---local t = tag.get("1") -- `t` is the tag with the name "1" on the focused output
---tag.toggle(t)
---```
---@param t TagConstructor
---@see TagHandle.toggle — The corresponding object method
function tag.toggle(t)
    local t = tag.get(t)

    if t then
        SendMsg({
            ToggleTag = {
                tag_id = t:id(),
            },
        })
    end
end

---Switch to a tag on the specified output, deactivating any other active tags on it.
---If the output is not specified, this uses the currently focused output instead.
---
---This is used to replicate what a traditional workspace is on some other Wayland compositors.
---
---### Examples
---```lua
---local op = output.get_by_name("DP-1")
---
---tag.switch_to("1")             -- Switch to tag 1 on the focused output
---
---tag.switch_to({ name = "1", output = "DP-1" }) -- Switch to tag 1 on "DP-1"
---tag.switch_to({ name = "1", output = op })     -- Same as above
---
--- -- Using a tag handle
---local t = tag.get_by_name("1")[1] -- `t` is the first tag with the name "1"
---tag.switch_to(t)
---```
---@param t TagConstructor
---@see TagHandle.switch_to — The corresponding object method
function tag.switch_to(t)
    local t = tag.get(t)

    if t then
        SendMsg({
            SwitchToTag = {
                tag_id = t:id(),
            },
        })
    end
end

---Set a layout for the tag on the specified output. If no output is provided, set it for the tag on the currently focused one.
---
---### Examples
---```lua
---local op = output.get_by_name("DP-1")
---
---tag.set_layout("1", "Dwindle")     -- Set tag 1 on the focused output to "Dwindle"
---
---tag.set_layout({ name = "1", output = "DP-1" }, "Dwindle") -- Set tag 1 on "DP-1" to "Dwindle"
---tag.set_layout({ name = "1", output = op }, "Dwindle")     -- Same as above
---
--- -- Using a tag handle
---local t = tag.get_by_name("1")[1] -- `t` is the first tag with the name "1"
---tag.set_layout(t, "Dwindle")
---```
---
---@param t TagConstructor
---@param layout Layout The layout.
---@see TagHandle.set_layout — The corresponding object method
function tag.set_layout(t, layout)
    local t = tag.get(t)

    if t then
        SendMsg({
            SetLayout = {
                tag_id = t:id(),
                layout = layout,
            },
        })
    end
end

---Get a tag with the specified name and optional output.
---
---If the output isn't specified, the focused one is used.
---
---If you have duplicate tags on an output, this returns the first one.
---If you need access to all duplicates, use `tag.get_on_output`, `tag.get_by_name`, or `tag.get_all`
---and filter for what you need.
---
---### Examples
---```lua
---local t = tag.get("1")
---local t = tag.get({ name = "1", output = "HDMI-A-0" })
---
---local op = output.get_by_name("DP-2")
---if op ~= nil then
---    local t = tag.get({ name = "Code", output = op })
---end
---```
---@param params TagConstructor
---@return TagHandle
---
---@see Tag.get_on_output — Get all tags on an output
---@see Tag.get_by_name — Get all tags with some name
---@see Tag.get_all — Get all tags
function tag.get(params)
    -- If creating from a tag object, just return the obj
    if params.id then
        return params --[[@as TagHandle]]
    end

    -- string passed in
    if type(params) == "string" then
        local op = require("output").get_focused()
        if op == nil then
            return create_tag("None")
        end

        local tags = tag.get_by_name(params)
        for _, t in pairs(tags) do
            if t:output() and t:output():name() == op:name() then
                return t
            end
        end

        return create_tag("None")
    end

    -- TagTable was passed in
    local params = params --[[@as TagTable]]
    local tag_name = params.name
    local op = params.output

    if op == nil then
        local o = require("output").get_focused()
        if o == nil then
            return create_tag("None")
        end
        op = o
    elseif type(op) == "string" then
        local o = require("output").get_by_name(op)
        if o == nil then
            return create_tag("None")
        end
        op = o
    end

    local tags = tag.get_by_name(tag_name)
    for _, t in pairs(tags) do
        if t:output() and t:output():name() == op:name() then
            return t
        end
    end

    return create_tag("None")
end

---Get all tags on the specified output.
---
---### Example
---```lua
---local op = output.get_focused()
---if op ~= nil then
---    local tags = tag.get_on_output(op) -- All tags on the focused output
---end
---```
---@param output OutputHandle
---@return TagHandle[]
---
---@see Output.tags — The corresponding object method
function tag.get_on_output(output)
    local response = Request({
        GetOutputProps = {
            output_name = output:name(),
        },
    })

    local tag_ids = response.RequestResponse.response.OutputProps.tag_ids

    ---@type TagHandle[]
    local tags = {}

    if tag_ids == nil then
        return tags
    end

    for _, tag_id in pairs(tag_ids) do
        table.insert(tags, create_tag(tag_id))
    end

    return tags
end

---Get all tags with this name across all outputs.
---
---### Example
---```lua
--- -- Given one monitor with the tags "OBS", "OBS", "VSCode", and "Spotify"...
---local tags = tag.get_by_name("OBS")
--- -- ...will have 2 tags in `tags`, while...
---local no_tags = tag.get_by_name("Firefox")
--- -- ...will have `no_tags` be empty.
---```
---@param name string The name of the tag(s) you want.
---@return TagHandle[]
function tag.get_by_name(name)
    local t_s = tag.get_all()

    ---@type TagHandle[]
    local tags = {}

    for _, t in pairs(t_s) do
        if t:name() == name then
            table.insert(tags, t)
        end
    end

    return tags
end

---Get all tags across all outputs.
---
---### Example
---```lua
--- -- With two monitors with the same tags: "1", "2", "3", "4", and "5"...
---local tags = tag.get_all()
--- -- ...`tags` should have 10 tags, with 5 pairs of those names across both outputs.
---```
---@return TagHandle[]
function tag.get_all()
    local response = Request("GetTags")

    local tag_ids = response.RequestResponse.response.Tags.tag_ids

    ---@type TagHandle[]
    local tags = {}

    for _, tag_id in pairs(tag_ids) do
        table.insert(tags, create_tag(tag_id))
    end

    return tags
end

---Get the specified tag's name.
---
---### Example
---```lua
--- -- Assuming the tag `Terminal` exists...
---print(tag.name(tag.get_by_name("Terminal")[1]))
--- -- ...should print `Terminal`.
---```
---@param t TagHandle
---@return string|nil
---@see TagHandle.name — The corresponding object method
function tag.name(t)
    local response = Request({
        GetTagProps = {
            tag_id = t:id(),
        },
    })
    local name = response.RequestResponse.response.TagProps.name
    return name
end

---Get whether or not the specified tag is active.
---@param t TagHandle
---@return boolean|nil
---@see TagHandle.active — The corresponding object method
function tag.active(t)
    local response = Request({
        GetTagProps = {
            tag_id = t:id(),
        },
    })
    local active = response.RequestResponse.response.TagProps.active
    return active
end

---Get the output the specified tag is on.
---@param t TagHandle
---@return OutputHandle
---@see Output.get_for_tag — The called function
---@see TagHandle.output — The corresponding object method
function tag.output(t)
    return require("output").get_for_tag(t)
end

---@class LayoutCycler
---@field next fun(output: (OutputHandle|OutputName)?) Change the first active tag on `output` to its next layout. If `output` is empty, the focused output is used.
---@field prev fun(output: (OutputHandle|OutputName)?) Change the first active tag on `output` to its previous layout. If `output` is empty, the focused output is used.

---Create a `LayoutCycler` to cycle layouts on tags.
---
---Given an array of layouts, this will create a table with two functions;
---one will cycle forward the layout for the active tag, and one will cycle backward.
---
--- ### Example
---```lua
---local layout_cycler = tag.layout_cycler({ "Dwindle", "Spiral", "MasterStack" })
---
---layout_cycler.next() -- Go to the next layout on the first active tag of the focused output
---layout_cycler.prev() -- Go to the previous layout on the first active tag of the focused output
---
---layout_cycler.next("DP-1") -- Do the above but on "DP-1" instead
---layout_cycler.prev(output.get_by_name("DP-1")) -- With an output handle
---```
---@param layouts Layout[] The available layouts.
---@return LayoutCycler layout_cycler A table with the functions `next` and `prev`, which will cycle layouts for the given tag.
function tag.layout_cycler(layouts)
    local indices = {}

    -- Return empty functions if layouts is empty
    if #layouts == 0 then
        return {
            next = function(_) end,
            prev = function(_) end,
        }
    end

    return {
        ---@param output (OutputHandle|OutputName)?
        next = function(output)
            if type(output) == "string" then
                output = require("output").get_by_name(output)
            end

            output = output or require("output").get_focused()

            if output == nil then
                return
            end

            local tags = output:tags()
            for _, tg in pairs(tags) do
                if tg:active() then
                    local id = tg:id()
                    if id == nil then
                        return
                    end

                    if #layouts == 1 then
                        indices[id] = 1
                    elseif indices[id] == nil then
                        indices[id] = 2
                    else
                        if indices[id] + 1 > #layouts then
                            indices[id] = 1
                        else
                            indices[id] = indices[id] + 1
                        end
                    end

                    tg:set_layout(layouts[indices[id]])
                    break
                end
            end
        end,

        ---@param output (OutputHandle|OutputName)?
        prev = function(output)
            if type(output) == "string" then
                output = require("output").get_by_name(output)
            end

            output = output or require("output").get_focused()

            if output == nil then
                return
            end

            local tags = output:tags()
            for _, tg in pairs(tags) do
                if tg:active() then
                    local id = tg:id()
                    if id == nil then
                        return
                    end

                    if #layouts == 1 then
                        indices[id] = 1
                    elseif indices[id] == nil then
                        indices[id] = #layouts - 1
                    else
                        if indices[id] - 1 < 1 then
                            indices[id] = #layouts
                        else
                            indices[id] = indices[id] - 1
                        end
                    end

                    tg:set_layout(layouts[indices[id]])
                    break
                end
            end
        end,
    }
end

return tag
