use omen::prelude::*;

view! {
    @Design
    pub ResponsiveView {
        Layout {
            Layout {
                Text("STUDIO WORKSPACE")
                    .fontSize(11).weight(.semibold).textColor("#E9AD76")
                Text("Project overview")
                    .fontSize(30).weight(.semibold).textColor("#EAEAED")
                Text("Track work from the first draft to release.")
                    .fontSize(14).textColor("#BCC4D0")
            }
            .spacing(8)

            Layout {
                Layout {
                    Text("Active projects").fontSize(13).textColor("#BCC4D0")
                    Text("12").fontSize(29).weight(.semibold).textColor("#EAEAED")
                    Text("3 added this month").fontSize(12).textColor("#E9AD76")
                }
                .spacing(8).padding(16).cornerRadius(14).background("#1B1B1E").fill()

                Layout {
                    Text("In review").fontSize(13).textColor("#BCC4D0")
                    Text("4").fontSize(29).weight(.semibold).textColor("#EAEAED")
                    Text("2 awaiting feedback").fontSize(12).textColor("#E9AD76")
                }
                .spacing(8).padding(16).cornerRadius(14).background("#1B1B1E").fill()

                Layout {
                    Text("Published").fontSize(13).textColor("#BCC4D0")
                    Text("8").fontSize(29).weight(.semibold).textColor("#EAEAED")
                    Text("Latest release on Friday").fontSize(12).textColor("#E9AD76")
                }
                .spacing(8).padding(16).cornerRadius(14).background("#1B1B1E").fill()
            }
            .horizontal(.medium)
            .spacing(12)
        }
        .spacing(20)
        .padding(24)
        .frame(.infinity, .infinity)
        .background("#0D0D0F")
    }
}
