PORTNAME=	sudo-rs
PORTVERSION=	0.2.7
DISTVERSIONPREFIX=	v
PORTREVISION=	1
CATEGORIES=	security

MAINTAINER=	marc@trifectatech.org
COMMENT=	Allow others to run commands as root
WWW=		https://trifectatech.org/initiatives/privilege-boundary/

LICENSE=	APACHE20 MIT
LICENSE_COMB=	dual
LICENSE_FILE_APACHE20=	${WRKSRC}/LICENSE-APACHE
LICENSE_FILE_MIT=	${WRKSRC}/LICENSE-MIT

FLAVORS=		default coexist
FLAVOR?=		${FLAVORS:[1]}
coexist_PKGNAMESUFFIX=	-coexist
coexist_DESCR=		pkg-descr-coexist

USES=		cargo

USE_GITHUB=	yes
GH_ACCOUNT=	trifectatechfoundation

CARGO_CRATES=	diff-0.1.13 \
		glob-0.3.2 \
		libc-0.2.174 \
		log-0.4.27 \
		pretty_assertions-1.4.1 \
		yansi-1.0.1

.if !defined(USER) || ${USER} == root
CARGO_TEST=	no
.endif

.if ${FLAVOR} == default
CONFLICTS_INSTALL=	sudo
RS_SUFFIX=
PLIST_SUB+=		NO_COEXIST=""
.else
RS_SUFFIX=		-rs
PLIST_SUB+=		NO_COEXIST="@comment "
.endif
PLIST_SUB+=		RS_SUFFIX=${RS_SUFFIX}

PORTDOCS=	CHANGELOG.md CONTRIBUTING.md COPYRIGHT LICENSE-* README.md \
		SECURITY.md

OPTIONS_DEFINE=	DOCS

post-install:
	${MV} ${STAGEDIR}${PREFIX}/bin/visudo ${STAGEDIR}${PREFIX}/sbin/visudo${RS_SUFFIX}
	${INSTALL_MAN} ${WRKSRC}/docs/man/sudo.8.man ${STAGEDIR}${PREFIX}/share/man/man8/sudo${RS_SUFFIX}.8
	${INSTALL_MAN} ${WRKSRC}/docs/man/visudo.8.man ${STAGEDIR}${PREFIX}/share/man/man8/visudo${RS_SUFFIX}.8
	${INSTALL_MAN} ${WRKSRC}/docs/man/sudoers.5.man ${STAGEDIR}${PREFIX}/share/man/man5/sudoers${RS_SUFFIX}.5
	${MKDIR} ${STAGEDIR}${PREFIX}/etc/sudoers.d
	${MKDIR} ${STAGEDIR}${PREFIX}/etc/pam.d
	${MKDIR} ${STAGEDIR}/var/run/sudo
	${MKDIR} ${STAGEDIR}/var/db/sudo
# we are not going to try to replace 'su', since that is part of FreeBSD itself
	${RM} ${STAGEDIR}${PREFIX}/bin/su
.if ${FLAVOR} == default
	${INSTALL_DATA} ${FILESDIR}/pam.conf ${STAGEDIR}${PREFIX}/etc/pam.d/sudo.default
	${INSTALL_DATA} ${FILESDIR}/sudoers ${STAGEDIR}${PREFIX}/etc/sudoers.dist
.else
	${MV} ${STAGEDIR}${PREFIX}/bin/sudo ${STAGEDIR}${PREFIX}/bin/sudo${RS_SUFFIX}
.endif

post-install-DOCS-on:
	${MKDIR} ${STAGEDIR}${DOCSDIR}
	cd ${WRKSRC} && ${INSTALL_DATA} ${PORTDOCS} ${STAGEDIR}${DOCSDIR}

.include <bsd.port.mk>
