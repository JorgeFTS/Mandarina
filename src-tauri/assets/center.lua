-- Filtro de pandoc para los docx que crea Mandarina. Traduce dos cosas del HTML de un README que el
-- escritor de docx ignora:
--  * <div align="center">: se le da el estilo "Centered", definido (centrado) en assets/reference-a4.docx.
--    Al pasar de Markdown a HTML, pandoc escribe el atributo como "data-align", así que se miran los dos.
--  * <div style="page-break-before: always"> (y page-break-after): salto de página.
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
    -- Un div vacío solo servía para el salto: no se deja
    if #el.content > 0 then table.insert(blocks, el) end
    if after then table.insert(blocks, PAGE_BREAK) end
    return blocks
  end
  if changed then return el end
end
