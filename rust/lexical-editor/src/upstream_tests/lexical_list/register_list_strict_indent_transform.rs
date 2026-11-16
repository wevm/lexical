//! Ports of `packages/lexical-list/src/__tests__/unit/registerListStrictIndentTransform.test.ts`.

use super::support::prettify_html;
use crate::html::{generate_html_from_nodes, generate_nodes_from_dom};
use crate::list::register_list_strict_indent_transform;
use crate::upstream_tests::support::{initialize_unit_test, update};
use crate::{CreateEditorArgs, get_root};

const OUTPUT: &str = r#"
        <ul>
          <li value="1">
            <span style="white-space: pre-wrap;">0</span>
            <ul>
              <li value="1">
                <span style="white-space: pre-wrap;">1</span>
                <ul>
                  <li value="1">
                    <span style="white-space: pre-wrap;">2</span>
                    <ul>
                      <li value="1">
                        <span style="white-space: pre-wrap;">3</span>
                      </li>
                    </ul>
                  </li>
                </ul>
              </li>
            </ul>
          </li>
        </ul>
      "#;

const CASES: [&str; 3] = [
    r#"
          <ul>
            <li>
              <span style="white-space: pre-wrap;">0</span>
            </li>
            <li>
              <ul>
                <li>
                  <ul>
                    <li>
                      <ul>
                        <li>
                          <!-- Indent: 3 -->
                          <span style="white-space: pre-wrap;">1</span>
                        </li>
                      </ul>
                    </li>
                  </ul>
                </li>
                <li>
                  <ul>
                    <li>
                      <!-- Indent: 2 -->
                      <span style="white-space: pre-wrap;">2</span>
                    </li>
                    <li>
                      <ul>
                        <li>
                          <!-- Indent: 3 -->
                          <span style="white-space: pre-wrap;">3</span>
                        </li>
                      </ul>
                    </li>
                  </ul>
                </li>
              </ul>
            </li>
          </ul>
        "#,
    r#"
          <ul>
            <li>
              <span style="white-space: pre-wrap;">0</span>
            </li>
            <li>
              <ul>
                <li>
                  <ul>
                    <li>
                      <!-- Indent: 2 -->
                      <span style="white-space: pre-wrap;">1</span>
                    </li>
                  </ul>
                </li>
                <li>
                  <ul>
                    <li>
                      <ul>
                        <li>
                          <ul>
                            <li>
                              <!-- Indent: 4 -->
                              <span style="white-space: pre-wrap;">2</span>
                            </li>
                          </ul>
                        </li>
                      </ul>
                    </li>
                    <li>
                      <ul>
                        <li>
                          <!-- Indent: 3 -->
                          <span style="white-space: pre-wrap;">3</span>
                        </li>
                      </ul>
                    </li>
                  </ul>
                </li>
              </ul>
            </li>
          </ul>
        "#,
    r#"
          <ul>
            <li>
              <span style="white-space: pre-wrap;">0</span>
            </li>
            <li>
              <ul>
                <li>
                  <ul>
                    <li>
                      <!-- Indent: 2 -->
                      <span style="white-space: pre-wrap;">1</span>
                    </li>
                  </ul>
                </li>
                <li>
                  <ul>
                    <li>
                      <ul>
                        <li>
                          <ul>
                            <li>
                              <!-- Indent: 4 -->
                              <span style="white-space: pre-wrap;">2</span>
                            </li>
                          </ul>
                        </li>
                      </ul>
                    </li>
                    <li>
                      <ul>
                        <li>
                          <ul>
                            <li>
                              <ul>
                                <li>
                                  <!-- Indent: 5 -->
                                  <span style="white-space: pre-wrap;">3</span>
                                </li>
                              </ul>
                            </li>
                          </ul>
                        </li>
                      </ul>
                    </li>
                  </ul>
                </li>
              </ul>
            </li>
          </ul>
        "#,
];

// upstream: lexical-list/src/__tests__/unit/registerListStrictIndentTransform.test.ts::Lexical List StrictIndentTransform tests > applyStrictListIndentation
#[test]
fn apply_strict_list_indentation() {
    let editor = initialize_unit_test(CreateEditorArgs {
        namespace: "test".into(),
        ..CreateEditorArgs::default()
    });
    let _ = register_list_strict_indent_transform(&editor);
    for input in CASES {
        let inner = editor.clone();
        update(&editor, move || {
            let root = get_root();
            root.clear()?;
            let dom = crate::dom::parse_from_string(input);
            let nodes = generate_nodes_from_dom(&inner, &dom)?;
            root.append(&nodes)
        });
        let html = editor
            .read(|| generate_html_from_nodes(&editor, None).unwrap())
            .unwrap();
        assert_eq!(prettify_html(&html), prettify_html(OUTPUT));
    }
}
