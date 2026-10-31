use std::error::Error;
use vesta_gpui::{ElementId, InputTarget, Rect, UiWindowId};
use vesta_gpui_widgets::PointerEditOutcome;
use vesta_gpui_workspace::{
    PaneHeader, PaneHeaderCommand, SplitResizeError, SplitResizeSession, WorkspaceProjectionError,
    WorkspaceViewInteraction, workspace_view_with_header,
};
use vesta_workspace::{ContainerRef, SplitAxis, SplitId};

#[test]
fn projection_and_resize_failures_remain_typed_at_the_host_boundary() {
    fn host_resize(ratio: f32) -> Result<SplitResizeSession, Box<dyn Error + Send + Sync>> {
        Ok(SplitResizeSession::new(
            InputTarget {
                window: UiWindowId(7),
                element: ElementId::new(11),
            },
            vesta_gpui_workspace::DockSplitFrame {
                container: ContainerRef::Main,
                split: SplitId(3),
                axis: SplitAxis::Row,
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 800.0,
                    height: 600.0,
                },
                flow_direction: vesta_gpui::FlowDirection::LeftToRight,
            },
            ratio,
        )?)
    }
    let failure = host_resize(f32::NAN).expect_err("invalid ratio rejected");
    assert_eq!(
        failure.downcast_ref::<SplitResizeError>(),
        Some(&SplitResizeError::InvalidRatio)
    );
    assert!(failure.source().is_none());
    assert!(!failure.to_string().is_empty());
    let mut resize = host_resize(0.52).expect("valid setup after rejection");
    assert_eq!(resize.container(), ContainerRef::Main);
    assert_eq!(resize.split(), SplitId(3));
    assert!(resize.finish(UiWindowId(8), 0.91, false).is_none());
    assert_eq!(
        resize.finish(UiWindowId(7), 0.91, false),
        Some(PointerEditOutcome::Cancel { restore: 0.52 })
    );
    assert!(resize.finish(UiWindowId(7), 0.91, true).is_none());
    assert_eq!(
        SplitResizeSession::new(
            resize.target(),
            vesta_gpui_workspace::DockSplitFrame {
                container: ContainerRef::Main,
                split: SplitId(3),
                axis: SplitAxis::Row,
                rect: Rect::new(f32::MAX, 0.0, f32::MAX, 600.0),
                flow_direction: vesta_gpui::FlowDirection::LeftToRight
            },
            0.5
        )
        .expect_err("overflowed edges"),
        SplitResizeError::InvalidGeometry
    );
    fn host_header() -> Result<(), Box<dyn Error + Send + Sync>> {
        use vesta_workspace::{Pane, PaneId, WorkspaceModel, WorkspaceNode};
        let pane = PaneId(1);
        let model = WorkspaceModel::with_tree(WorkspaceNode::leaf(pane), [Pane::new(pane)])?;
        let scope = vesta_gpui::ElementIdAllocator
            .allocate_scope()
            .expect("fixture scope");
        workspace_view_with_header(
            scope,
            &model,
            vesta_gpui_widgets::WidgetTheme::dark(),
            WorkspaceViewInteraction::default(),
            None,
            |_| PaneHeader {
                commands: vec![PaneHeaderCommand {
                    id: 0,
                    caption: "+".into(),
                    name: "invalid".into(),
                    enabled: true,
                }],
                ..PaneHeader::default()
            },
            |_| None,
        )?;
        Ok(())
    }
    let failure = host_header().expect_err("invalid command rejected even on an empty pane");
    assert_eq!(
        failure.downcast_ref::<WorkspaceProjectionError>(),
        Some(&WorkspaceProjectionError::InvalidHeaderCommandId {
            pane: vesta_workspace::PaneId(1),
            command: 0
        })
    );
}
