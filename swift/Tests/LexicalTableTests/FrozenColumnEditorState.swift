import Lexical

// The editor state upstream's "Toggle frozen first column ON/OFF" expects
// (packages/lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx).
let frozenFirstColumnEditorState = """
  {
    "root": {
      "children": [
        {
          "children": [],
          "direction": null,
          "format": "",
          "indent": 0,
          "textFormat": 0,
          "textStyle": "",
          "type": "paragraph",
          "version": 1
        },
        {
          "children": [
            {
              "children": [
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 3,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                },
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 1,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                },
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 1,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                },
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 1,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                }
              ],
              "direction": null,
              "format": "",
              "indent": 0,
              "type": "tablerow",
              "version": 1
            },
            {
              "children": [
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 2,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                },
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 0,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                },
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 0,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                },
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 0,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                }
              ],
              "direction": null,
              "format": "",
              "indent": 0,
              "type": "tablerow",
              "version": 1
            },
            {
              "children": [
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 2,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                },
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 0,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                },
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 0,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                },
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 0,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                }
              ],
              "direction": null,
              "format": "",
              "indent": 0,
              "type": "tablerow",
              "version": 1
            },
            {
              "children": [
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 2,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                },
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 0,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                },
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 0,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                },
                {
                  "backgroundColor": null,
                  "children": [
                    {
                      "children": [],
                      "direction": null,
                      "format": "",
                      "indent": 0,
                      "textFormat": 0,
                      "textStyle": "",
                      "type": "paragraph",
                      "version": 1
                    }
                  ],
                  "colSpan": 1,
                  "direction": null,
                  "format": "",
                  "headerState": 0,
                  "indent": 0,
                  "rowSpan": 1,
                  "type": "tablecell",
                  "version": 1
                }
              ],
              "direction": null,
              "format": "",
              "indent": 0,
              "type": "tablerow",
              "version": 1
            }
          ],
          "direction": null,
          "format": "",
          "frozenColumnCount": 1,
          "indent": 0,
          "type": "table",
          "version": 1
        }
      ],
      "direction": null,
      "format": "",
      "indent": 0,
      "type": "root",
      "version": 1
    }
  }
  """
