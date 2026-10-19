-- Pandoc filter for the docx files that Mandarina creates. It translates things from the HTML of a README
-- that the docx writer ignores, and it sets the width of the tables:
--  * <div align="center">: it gets the "Centered" style, defined (centered) in assets/reference-a4.docx.
--    When going from Markdown to HTML pandoc writes the attribute as "data-align", so both are checked.
--  * <div style="page-break-before: always"> (and page-break-after): a page break.
--  * Tables with no explicit widths: each column gets a width that follows its content (see Table below).
local PAGE_BREAK = pandoc.RawBlock("openxml", '<w:p><w:r><w:br w:type="page"/></w:r></w:p>')

local function has_style(el, pattern)
  local style = el.attributes["style"] or ""
  return style:find(pattern) ~= nil
end

function Div(el)
  local changed = false
  local align = el.attributes["align"] or el.attributes["data-align"]
  if align == "center" then
    el.attributes["custom-style"] = "Centered"
    changed = true
  end
  local before = has_style(el, "page%-break%-before%s*:%s*always")
  local after = has_style(el, "page%-break%-after%s*:%s*always")
  if before or after then
    local blocks = {}
    if before then table.insert(blocks, PAGE_BREAK) end
    -- An empty div only served for the break: it is not kept
    if #el.content > 0 then table.insert(blocks, el) end
    if after then table.insert(blocks, PAGE_BREAK) end
    return blocks
  end
  if changed then return el end
end

local function length_of(text)
  return utf8.len(text) or #text
end

-- A table with no widths (a raw HTML table, or a Markdown one once the reader is told not to use the
-- dashes of the separator line) would get equal columns. Each column gets a weight from its content:
-- the average length of its cells, but never less than its longest word (so words are not broken) and
-- never more than 45 characters (so one long column does not squeeze the rest). Tables that already
-- have explicit widths (grid tables, for example) are left as they are.
function Table(tbl)
  local n = #tbl.colspecs
  if n < 2 then return nil end
  for _, spec in ipairs(tbl.colspecs) do
    if spec[2] then return nil end
  end
  local total, longest, count = {}, {}, {}
  for i = 1, n do
    total[i], longest[i], count[i] = 0, 0, 0
  end
  local function add_row(row)
    local col = 1
    for _, cell in ipairs(row.cells) do
      local span = cell.col_span or 1
      if span == 1 and col <= n then
        local text = pandoc.utils.stringify(cell.contents)
        total[col] = total[col] + length_of(text)
        count[col] = count[col] + 1
        for word in text:gmatch("%S+") do
          longest[col] = math.max(longest[col], length_of(word))
        end
      end
      col = col + span
    end
  end
  for _, row in ipairs(tbl.head.rows) do add_row(row) end
  for _, body in ipairs(tbl.bodies) do
    for _, row in ipairs(body.body) do add_row(row) end
  end
  local weights, sum = {}, 0
  for i = 1, n do
    local average = count[i] > 0 and total[i] / count[i] or 8
    weights[i] = math.max(math.min(average, 45), math.min(longest[i] * 1.3 + 3, 30), 6)
    sum = sum + weights[i]
  end
  for i = 1, n do
    tbl.colspecs[i] = { tbl.colspecs[i][1], weights[i] / sum }
  end
  return tbl
end
